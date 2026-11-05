//! Emulator configuration and RetroAchievements.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::app::AppPaths;
use super::CommandResult;
use crate::db::repo::achievements::{self, AchievementRow};
use crate::db::repo::game_sources::GameSource;
use crate::db::repo::{game_sources, settings};
use crate::db::{new_id, Db};
use crate::error::{AppError, AppResult};
use crate::metadata::retroachievements as ra;
use crate::models::SourceKind;
use crate::secrets::{SecretKind, Secrets};
use crate::sources::emulator::{self, EmulatorConfig};

const RA_USER_KEY: &str = "ra.username";

#[tauri::command]
#[specta::specta]
pub fn emulators_get(db: tauri::State<'_, Db>) -> Vec<EmulatorConfig> {
    emulator::load(&db)
}

/// Saves the whole list (each entry validated). New entries get an id.
#[tauri::command]
#[specta::specta]
pub fn emulators_set(
    db: tauri::State<'_, Db>,
    emulators: Vec<EmulatorConfig>,
) -> CommandResult<Vec<EmulatorConfig>> {
    let mut list = Vec::new();
    for mut e in emulators {
        e.extensions = e
            .extensions
            .iter()
            .map(|x| x.trim().trim_start_matches('.').to_lowercase())
            .filter(|x| !x.is_empty())
            .collect();
        emulator::validate(&e)?;
        if e.id.trim().is_empty() {
            e.id = new_id();
        }
        list.push(e);
    }
    settings::set(&db.conn(), emulator::SETTINGS_KEY, &list)?;
    Ok(list)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleOption {
    pub id: u32,
    pub name: String,
    /// Achievements matched automatically from the ROM file.
    pub auto_match: bool,
}

#[tauri::command]
#[specta::specta]
pub fn ra_consoles() -> Vec<ConsoleOption> {
    ra::CONSOLES
        .iter()
        .map(|(id, name)| ConsoleOption {
            id: *id,
            name: name.to_string(),
            auto_match: emulator::hashable_console(*id),
        })
        .collect()
}

#[tauri::command]
#[specta::specta]
pub fn ra_username_get(db: tauri::State<'_, Db>) -> Option<String> {
    settings::get(&db.conn(), RA_USER_KEY).ok().flatten()
}

#[tauri::command]
#[specta::specta]
pub fn ra_username_set(db: tauri::State<'_, Db>, username: Option<String>) -> CommandResult<()> {
    match username.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => settings::set(&db.conn(), RA_USER_KEY, u)?,
        None => settings::delete(&db.conn(), RA_USER_KEY)?,
    }
    Ok(())
}

/// Sets (or clears) the RetroAchievements game id of an emulated game.
#[tauri::command]
#[specta::specta]
pub fn game_set_ra_id(
    db: tauri::State<'_, Db>,
    game_id: String,
    ra_id: Option<u32>,
) -> CommandResult<()> {
    let conn = db.conn();
    let source = game_sources::list_for_game(&conn, &game_id)?
        .into_iter()
        .find(|s| s.kind == SourceKind::Emulator)
        .ok_or_else(|| AppError::Invalid("not an emulated game".into()))?;
    conn.execute(
        "UPDATE game_sources SET ra_game_id = ?2, achievements_synced_at = NULL WHERE id = ?1",
        rusqlite::params![source.id, ra_id.map(i64::from)],
    )
    .map_err(AppError::from)?;
    Ok(())
}

/// Outcome of a RetroAchievements lookup for one emulated game.
pub enum RaResult {
    Rows(Vec<AchievementRow>),
    NeedsKey,
    NotMatched,
}

fn ra_id_of(conn: &rusqlite::Connection, source_id: &str) -> AppResult<Option<u32>> {
    let id: Option<i64> = conn.query_row(
        "SELECT ra_game_id FROM game_sources WHERE id = ?1",
        [source_id],
        |r| r.get(0),
    )?;
    Ok(id.and_then(|i| u32::try_from(i).ok()))
}

/// Finds the RA game (stored id, else ROM hash) and fetches the progress.
pub fn fetch_ra(app: &AppHandle, source: &GameSource) -> AppResult<RaResult> {
    let db = app.state::<Db>();
    let user: Option<String> = settings::get(&db.conn(), RA_USER_KEY)?;
    let key = app
        .state::<Secrets>()
        .get(SecretKind::RetroAchievementsApiKey)?;
    let (Some(user), Some(key)) = (user, key) else {
        return Ok(RaResult::NeedsKey);
    };

    let mut game_id = ra_id_of(&db.conn(), &source.id)?;
    if game_id.is_none() {
        let config = emulator::load(&db).into_iter().find(|c| {
            source
                .exe_path
                .as_deref()
                .is_some_and(|e| e.eq_ignore_ascii_case(&c.exe_path))
        });
        if let (Some(console), Some(rom)) = (
            config.and_then(|c| c.console_id),
            source.external_id.as_deref(),
        ) {
            if emulator::hashable_console(console) {
                let data = std::fs::read(rom)?;
                if let Some(hash) = emulator::ra_hash(console, &data) {
                    let cache = app.state::<AppPaths>().data_dir.join("ra");
                    let list = ra::game_list(&cache, &key, console)?;
                    game_id = ra::find_by_hash(&list, &hash);
                    if let Some(id) = game_id {
                        db.conn().execute(
                            "UPDATE game_sources SET ra_game_id = ?2 WHERE id = ?1",
                            rusqlite::params![source.id, i64::from(id)],
                        )?;
                    }
                }
            }
        }
    }
    match game_id {
        Some(id) => Ok(RaResult::Rows(ra::progress(&key, &user, id)?)),
        None => Ok(RaResult::NotMatched),
    }
}

/// Stores RA rows in the shared achievements cache.
pub fn store_ra(db: &Db, source_id: &str, rows: &[AchievementRow]) -> AppResult<()> {
    achievements::replace_for_source(&mut db.conn(), source_id, Some(rows))
}
