//! Source status, synchronisation and per-source settings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::app::AppPaths;
use super::CommandResult;
use crate::db::repo::settings;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::library::sync::{self, ArtworkJob, SyncReport};
use crate::metadata::artwork;
use crate::models::SourceKind;
use crate::secrets::{SecretKind, Secrets};
use crate::sources::epic::{self, EpicSource};
use crate::sources::gog::{self, GogSource};
use crate::sources::steam::{local, SteamConfig, SteamSource};
use crate::sources::{scan_all, Source, SourceStatus};

pub const LIBRARY_CHANGED_EVENT: &str = "library-changed";
const STEAM_ID_KEY: &str = "steam.steam_id";

/// Sync bookkeeping kept in Tauri state.
#[derive(Default)]
pub struct SyncState {
    running: AtomicBool,
    artwork_running: AtomicBool,
    metadata_running: AtomicBool,
    last: Mutex<HashMap<SourceKind, SyncReport>>,
}

/// Steam settings resolved from the registry, the settings table and the vault.
pub(crate) fn steam_config(db: &Db, secrets: &Secrets) -> SteamConfig {
    let root = local::locate_steam();
    let configured: Option<String> = settings::get(&db.conn(), STEAM_ID_KEY).ok().flatten();
    let steam_id = configured
        .as_deref()
        .and_then(local::parse_steamid64)
        .or_else(|| {
            root.as_deref()
                .and_then(|r| local::login_users(r).first().map(|a| a.steam_id))
        });
    let api_key = match secrets.get(SecretKind::SteamWebApiKey) {
        Ok(key) => key,
        Err(e) => {
            tracing::warn!(error = %e, "cannot read Steam API key from the vault");
            None
        }
    };
    SteamConfig {
        root,
        steam_id,
        api_key,
    }
}

/// Sources scanned by the sync, in display order.
pub const SCANNED_SOURCES: &[SourceKind] = &[
    SourceKind::Steam,
    SourceKind::Gog,
    SourceKind::Epic,
    SourceKind::Battlenet,
    SourceKind::Ubisoft,
    SourceKind::Ea,
    SourceKind::Riot,
    SourceKind::Emulator,
];

fn enabled_key(kind: SourceKind) -> String {
    format!("sources.{}.enabled", kind.as_str())
}

/// Steam is on by default; other launchers are off until the user enables
/// them (Settings › Sources or the import wizard), so nothing is imported
/// without being asked.
pub fn is_enabled(db: &Db, kind: SourceKind) -> bool {
    settings::get::<bool>(&db.conn(), &enabled_key(kind))
        .ok()
        .flatten()
        .unwrap_or(matches!(kind, SourceKind::Steam | SourceKind::Emulator))
}

pub fn set_enabled(db: &Db, kind: SourceKind, enabled: bool) -> AppResult<()> {
    settings::set(&db.conn(), &enabled_key(kind), &enabled)
}

fn make_source(db: &Db, secrets: &Secrets, kind: SourceKind) -> Option<Box<dyn Source>> {
    match kind {
        SourceKind::Steam => Some(Box::new(SteamSource::new(steam_config(db, secrets)))),
        SourceKind::Gog => Some(Box::new(GogSource::new(gog::locate_db()))),
        SourceKind::Epic => Some(Box::new(EpicSource::new(epic::locate_manifests()))),
        SourceKind::Emulator => Some(Box::new(crate::sources::emulator::EmulatorSource::new(
            crate::sources::emulator::load(db),
        ))),
        SourceKind::Battlenet | SourceKind::Ubisoft | SourceKind::Ea | SourceKind::Riot => {
            Some(Box::new(crate::sources::others::OtherSource::new(kind)))
        }
        _ => None,
    }
}

/// Every source the app knows. Launchers that are not installed are kept in
/// the list: they report `NotInstalled` and are skipped by the scan.
pub fn build_sources(db: &Db, secrets: &Secrets, only_enabled: bool) -> Vec<Box<dyn Source>> {
    SCANNED_SOURCES
        .iter()
        .filter(|k| !only_enabled || is_enabled(db, **k))
        .filter_map(|k| make_source(db, secrets, *k))
        .collect()
}

fn location(kind: SourceKind, db: &Db, secrets: &Secrets) -> Option<String> {
    let path = match kind {
        SourceKind::Steam => steam_config(db, secrets).root,
        SourceKind::Gog => gog::locate_db(),
        SourceKind::Epic => epic::locate_manifests(),
        SourceKind::Riot => crate::sources::others::riot_client().map(std::path::PathBuf::from),
        _ => None,
    };
    path.map(|p| p.display().to_string())
}

/// Scans every source and writes the results. Runs on a blocking thread.
pub fn run_sync(app: &AppHandle) -> AppResult<Vec<SyncReport>> {
    let state = app.state::<SyncState>();
    if state.running.swap(true, Ordering::SeqCst) {
        return Err(AppError::Invalid(
            "a synchronisation is already running".into(),
        ));
    }
    let result = sync_inner(app);
    state.running.store(false, Ordering::SeqCst);
    result
}

fn sync_inner(app: &AppHandle) -> AppResult<Vec<SyncReport>> {
    let db = app.state::<Db>();
    let secrets = app.state::<Secrets>();
    let data_dir = app.state::<AppPaths>().data_dir.clone();

    let sources = build_sources(&db, &secrets, true);
    let mut reports = Vec::new();
    let mut jobs: Vec<ArtworkJob> = Vec::new();
    let ignored = crate::library::import::ignored(&db.conn())?;
    for scan in scan_all(&sources) {
        let scan = crate::library::import::without_ignored(scan, &ignored);
        let (report, mut source_jobs) = {
            let mut conn = db.conn();
            sync::apply_scan(&mut conn, &scan)?
        };
        tracing::info!(
            kind = ?report.kind, status = ?report.status, added = report.added,
            updated = report.updated, uninstalled = report.uninstalled, hidden = report.hidden,
            "source synced"
        );
        jobs.append(&mut source_jobs);
        reports.push(report);
    }

    match crate::library::non_games::hide_existing_once(&db.conn()) {
        Ok(0) => {}
        Ok(n) => tracing::info!(hidden = n, "existing playtests/betas/demos hidden"),
        Err(e) => tracing::warn!(error = %e, "could not hide non-game entries"),
    }

    let misses = artwork::MissCache::load(&data_dir);
    let copied = artwork::fill(&db, &data_dir, &jobs, false, &misses);
    tracing::info!(copied, pending = jobs.len(), "local artwork copied");

    {
        let state = app.state::<SyncState>();
        let mut last = state.last.lock().unwrap_or_else(|e| e.into_inner());
        for r in &reports {
            last.insert(r.kind, r.clone());
        }
    }
    let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
    spawn_artwork_downloads(app.clone(), data_dir, jobs);
    spawn_store_metadata(app.clone());
    Ok(reports)
}

/// Seconds between two Steam store requests (~200 allowed per 5 minutes).
const STORE_DELAY: std::time::Duration = std::time::Duration::from_millis(1600);

/// Fetches genres and player modes of Steam games from the store, slowly,
/// in the background. Stops on rate limiting and resumes at the next sync.
fn spawn_store_metadata(app: AppHandle) {
    if app
        .state::<SyncState>()
        .metadata_running
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    std::thread::spawn(move || {
        let db = app.state::<Db>();
        let todo: Vec<(String, String, String)> = {
            let conn = db.conn();
            let mut stmt = match conn.prepare(
                "SELECT s.id, s.game_id, s.external_id FROM game_sources s JOIN games g ON g.id = s.game_id \
                 WHERE s.kind = 'steam' AND s.store_meta_synced_at IS NULL AND g.hidden = 0 \
                 ORDER BY s.source_last_played DESC",
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "store metadata: query failed");
                    app.state::<SyncState>().metadata_running.store(false, Ordering::SeqCst);
                    return;
                }
            };
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .and_then(|rows| rows.collect())
                .unwrap_or_default()
        };
        let mut done = 0;
        for (source_id, game_id, appid) in todo {
            match crate::metadata::genres::fetch_steam(&appid) {
                Ok(data) => {
                    let conn = db.conn();
                    if let Some((genres, modes)) = &data {
                        let _ = crate::db::repo::tags::set_auto(&conn, &game_id, "genre", genres);
                        let _ = crate::db::repo::tags::set_auto(&conn, &game_id, "mode", modes);
                    }
                    let _ = conn.execute(
                        "UPDATE game_sources SET store_meta_synced_at = ?2 WHERE id = ?1",
                        rusqlite::params![source_id, crate::db::now_rfc3339()],
                    );
                    done += 1;
                    if done % 25 == 0 {
                        let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
                    }
                }
                Err(AppError::Network {
                    status: Some(429), ..
                }) => {
                    tracing::info!(done, "store metadata: rate limited, will resume later");
                    break;
                }
                Err(e) => tracing::debug!(%appid, error = %e, "store metadata failed"),
            }
            std::thread::sleep(STORE_DELAY);
        }
        if done > 0 {
            tracing::info!(done, "store metadata fetched");
            let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
        }
        app.state::<SyncState>()
            .metadata_running
            .store(false, Ordering::SeqCst);
    });
}

/// Downloads the artwork still missing, in the background, then refreshes the UI.
fn spawn_artwork_downloads(app: AppHandle, data_dir: PathBuf, jobs: Vec<ArtworkJob>) {
    if app
        .state::<SyncState>()
        .artwork_running
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    std::thread::spawn(move || {
        let db = app.state::<Db>();
        let misses = artwork::MissCache::load(&data_dir);
        // Small batches so covers appear progressively.
        let mut total = 0;
        for chunk in jobs.chunks(12) {
            let stored = artwork::fill(&db, &data_dir, chunk, true, &misses);
            total += stored;
            if stored > 0 {
                let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
            }
        }
        misses.save();
        tracing::info!(downloaded = total, "artwork downloads finished");
        app.state::<SyncState>()
            .artwork_running
            .store(false, Ordering::SeqCst);
        // Then what the launchers did not provide (logos of non-Steam games…).
        super::artwork::spawn_autofill(app);
    });
}

#[tauri::command]
#[specta::specta]
pub async fn library_sync(app: AppHandle) -> CommandResult<Vec<SyncReport>> {
    let result = tauri::async_runtime::spawn_blocking(move || run_sync(&app))
        .await
        .map_err(|e| AppError::Invalid(format!("sync task failed: {e}")))?;
    Ok(result?)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceCard {
    pub kind: SourceKind,
    pub status: SourceStatus,
    /// Where the launcher was found, when relevant.
    pub location: Option<String>,
    pub enabled: bool,
    pub last_sync: Option<SyncReport>,
    pub syncing: bool,
}

#[tauri::command]
#[specta::specta]
pub fn sources_status(
    db: tauri::State<'_, Db>,
    secrets: tauri::State<'_, Secrets>,
    sync_state: tauri::State<'_, SyncState>,
) -> CommandResult<Vec<SourceCard>> {
    let last = sync_state.last.lock().unwrap_or_else(|e| e.into_inner());
    let syncing = sync_state.running.load(Ordering::SeqCst);
    Ok(build_sources(&db, &secrets, false)
        .into_iter()
        .map(|source| {
            let kind = source.kind();
            let last_sync = last.get(&kind).cloned();
            // A failed scan (e.g. Web API error) is only visible in the last report.
            let status = match &last_sync {
                Some(r) if matches!(r.status, SourceStatus::Error(_)) => r.status.clone(),
                _ => source.status(),
            };
            SourceCard {
                kind,
                status,
                location: location(kind, &db, &secrets),
                enabled: is_enabled(&db, kind),
                last_sync,
                syncing,
            }
        })
        .collect())
}

/// Turns a source on or off. Disabling keeps its games in the library.
#[tauri::command]
#[specta::specta]
pub fn source_set_enabled(
    db: tauri::State<'_, Db>,
    kind: SourceKind,
    enabled: bool,
) -> CommandResult<()> {
    if !SCANNED_SOURCES.contains(&kind) {
        return Err(AppError::Invalid(format!("{} cannot be toggled", kind.as_str())).into());
    }
    Ok(set_enabled(&db, kind, enabled)?)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SteamAccountInfo {
    pub steam_id: String,
    pub persona_name: Option<String>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SteamSettings {
    pub steam_path: Option<String>,
    /// SteamID64 in use (configured, or detected from loginusers.vdf).
    pub steam_id: Option<String>,
    /// True when the user chose the SteamID explicitly.
    pub steam_id_configured: bool,
    pub accounts: Vec<SteamAccountInfo>,
    pub api_key_present: bool,
}

#[tauri::command]
#[specta::specta]
pub fn steam_settings_get(
    db: tauri::State<'_, Db>,
    secrets: tauri::State<'_, Secrets>,
) -> CommandResult<SteamSettings> {
    let config = steam_config(&db, &secrets);
    let configured: Option<String> = settings::get(&db.conn(), STEAM_ID_KEY)?;
    let accounts = config
        .root
        .as_deref()
        .map(local::login_users)
        .unwrap_or_default()
        .into_iter()
        .map(|a| SteamAccountInfo {
            steam_id: a.steam_id.to_string(),
            persona_name: a.persona_name,
        })
        .collect();
    Ok(SteamSettings {
        steam_path: config.root.map(|p| p.display().to_string()),
        steam_id: config.steam_id.map(|id| id.to_string()),
        steam_id_configured: configured.is_some(),
        accounts,
        api_key_present: config.api_key.is_some(),
    })
}

/// Sets the SteamID64 to use; `None` goes back to automatic detection.
#[tauri::command]
#[specta::specta]
pub fn steam_settings_set(db: tauri::State<'_, Db>, steam_id: Option<String>) -> CommandResult<()> {
    let conn = db.conn();
    match steam_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => settings::delete(&conn, STEAM_ID_KEY)?,
        Some(text) => {
            let id = local::parse_steamid64(text)
                .ok_or_else(|| AppError::Invalid("invalid SteamID64".into()))?;
            settings::set(&conn, STEAM_ID_KEY, &id.to_string())?;
        }
    }
    Ok(())
}

/// Used by `launch_game` to give a clear error when Steam is missing.
pub fn steam_installed() -> bool {
    local::locate_steam().is_some()
}
