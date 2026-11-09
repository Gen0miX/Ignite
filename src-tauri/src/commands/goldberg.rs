//! Goldberg Emulator setup for local games (Achievements tab › Succès).

use std::path::Path;

use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::game_sources::{self, GameSource};
use crate::db::Db;
use crate::error::AppError;
use crate::secrets::{SecretKind, Secrets};
use crate::sources::goldberg::{self, GoldbergFind, GoldbergStatus};
use crate::sources::goldberg_settings::{self, GoldbergSettings};

fn local_source(db: &Db, game_id: &str) -> CommandResult<GameSource> {
    game_sources::list_for_game(&db.conn(), game_id)?
        .into_iter()
        .find(goldberg::is_eligible_source)
        .ok_or_else(|| AppError::Invalid("not a local game with an executable".into()).into())
}

#[tauri::command]
#[specta::specta]
pub async fn goldberg_status(app: AppHandle, game_id: String) -> CommandResult<GoldbergStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let source = local_source(&db, &game_id)?;
        let status = goldberg::status(&db.conn(), &source);
        Ok::<_, super::CommandError>(status)
    })
    .await
    .map_err(|e| {
        super::CommandError::from(AppError::Invalid(format!("goldberg task failed: {e}")))
    })?
}

/// Installs (or repairs) Goldberg, writes the appid and regenerates the
/// achievement definitions. `force` reinstalls the DLL.
#[tauri::command]
#[specta::specta]
pub async fn goldberg_setup(
    app: AppHandle,
    game_id: String,
    appid: u32,
    force: bool,
) -> CommandResult<GoldbergStatus> {
    if appid == 0 {
        return Err(AppError::Invalid("invalid Steam appid".into()).into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let source = local_source(&db, &game_id)?;
        let key = app
            .state::<Secrets>()
            .get(SecretKind::SteamWebApiKey)
            .ok()
            .flatten();
        goldberg::install(&db.conn(), &source, Some(appid), force)?;
        let mut status = goldberg::configure(&db.conn(), &source, appid, key.as_deref())?;
        // New definitions: their Steam rarity is not cached yet.
        goldberg::refresh_cache(&mut db.conn(), &source)?;
        goldberg::refresh_rarity(&db, &source, true);
        if key.is_none() && !status.definitions_ready {
            status
                .issues
                .push(crate::sources::goldberg::GoldbergIssue::NeedsSteamKey);
        }
        Ok::<_, super::CommandError>(status)
    })
    .await
    .map_err(|e| {
        super::CommandError::from(AppError::Invalid(format!("goldberg task failed: {e}")))
    })?
}

/// Removes the configured appid (the game stays installed).
#[tauri::command]
#[specta::specta]
pub fn goldberg_clear_appid(app: AppHandle, game_id: String) -> CommandResult<GoldbergStatus> {
    let db = app.state::<Db>();
    let source = local_source(&db, &game_id)?;
    game_sources::set_goldberg_appid(&db.conn(), &source.id, None)?;
    // Also drop the file, or it would be picked up again as the appid.
    if let Some(dir) = source
        .exe_path
        .as_deref()
        .and_then(|e| Path::new(e).parent())
    {
        let _ = std::fs::remove_file(
            crate::sources::goldberg::settings_dir_for(dir).join("steam_appid.txt"),
        );
    }
    let status = goldberg::status(&db.conn(), &source);
    Ok(status)
}

/// Creates the saves folder even before a first achievement is earned.
#[tauri::command]
#[specta::specta]
pub fn goldberg_open_saves(app: AppHandle, game_id: String) -> CommandResult<String> {
    let db = app.state::<Db>();
    let source = local_source(&db, &game_id)?;
    let status = goldberg::status(&db.conn(), &source);
    let appid = status
        .appid
        .ok_or_else(|| AppError::Invalid("no appid configured".into()))?;
    let dir = goldberg::saves_root().join(appid.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Invalid(format!("saves folder: {e}")))?;
    let path = dir.display().to_string();
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|e| AppError::Invalid(format!("cannot open saves folder: {e}")))?;
    Ok(path)
}

/// Looks for achievements already unlocked for the configured appid in the
/// usual Goldberg save locations (GSE saves, classic Goldberg and Public
/// Documents), then announces each one so the user gets a notification.
#[tauri::command]
#[specta::specta]
pub async fn goldberg_find_saves(app: AppHandle, game_id: String) -> CommandResult<GoldbergFind> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let source = local_source(&db, &game_id)?;
        let status = goldberg::status(&db.conn(), &source);
        let appid = status
            .appid
            .ok_or_else(|| AppError::Invalid("no appid configured".into()))?;
        let found = goldberg::find_saves(appid);
        // Without the database lock: popups open a window.
        goldberg::announce_unlocks(&app, &source, &found.unlocked, false);
        Ok::<_, super::CommandError>(found)
    })
    .await
    .map_err(|e| {
        super::CommandError::from(AppError::Invalid(format!("goldberg task failed: {e}")))
    })?
}

/// Folder whose Goldberg configs are edited: the game's `steam_settings`,
/// or the global settings folder when `game_id` is `None`.
fn settings_folder(
    app: &AppHandle,
    game_id: Option<&str>,
) -> CommandResult<Option<std::path::PathBuf>> {
    let Some(game_id) = game_id else {
        return Ok(None);
    };
    let db = app.state::<Db>();
    let source = local_source(&db, game_id)?;
    let dir = goldberg::game_settings_dir(&source)
        .ok_or_else(|| AppError::Invalid("game folder not found".into()))?;
    Ok(Some(dir))
}

/// Goldberg options of a game (`game_id`) or the global ones (`None`).
#[tauri::command]
#[specta::specta]
pub fn goldberg_settings_get(
    app: AppHandle,
    game_id: Option<String>,
) -> CommandResult<GoldbergSettings> {
    let folder = settings_folder(&app, game_id.as_deref())?;
    Ok(goldberg_settings::view(folder.as_deref()))
}

/// Sets one Goldberg option, or clears it with `value = None` (back to the
/// global value for a game, to Goldberg's default globally).
#[tauri::command]
#[specta::specta]
pub fn goldberg_setting_set(
    app: AppHandle,
    game_id: Option<String>,
    id: String,
    value: Option<String>,
) -> CommandResult<GoldbergSettings> {
    let folder = settings_folder(&app, game_id.as_deref())?;
    let target = folder.clone().unwrap_or_else(goldberg_settings::global_dir);
    goldberg_settings::set(&target, &id, value.as_deref(), goldberg::OVERLAY_TEMPLATE)?;
    Ok(goldberg_settings::view(folder.as_deref()))
}
