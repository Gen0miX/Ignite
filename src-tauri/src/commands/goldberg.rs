//! Goldberg Emulator setup for local games (Achievements tab › Succès).

use std::path::Path;

use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::game_sources::{self, GameSource};
use crate::db::Db;
use crate::error::AppError;
use crate::secrets::{SecretKind, Secrets};
use crate::sources::goldberg::{self, GoldbergFind, GoldbergStatus};

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
        let conn = db.conn();
        let status = goldberg::status(&conn, &source);
        let appid = status
            .appid
            .ok_or_else(|| AppError::Invalid("no appid configured".into()))?;
        let found = goldberg::find_saves(appid);
        goldberg::announce_unlocks(&app, &conn, &source, &found.unlocked);
        Ok::<_, super::CommandError>(found)
    })
    .await
    .map_err(|e| {
        super::CommandError::from(AppError::Invalid(format!("goldberg task failed: {e}")))
    })?
}

/// Shows a test notification through the same path as achievement toasts.
#[tauri::command]
#[specta::specta]
pub fn goldberg_test_toast(app: AppHandle, title: String, body: String) -> CommandResult<()> {
    Ok(goldberg::test_toast(&app, &title, &body)?)
}
