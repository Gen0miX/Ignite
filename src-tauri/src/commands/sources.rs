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
use crate::sources::steam::{local, SteamConfig, SteamSource};
use crate::sources::{scan_all, Source, SourceStatus};

pub const LIBRARY_CHANGED_EVENT: &str = "library-changed";
const STEAM_ID_KEY: &str = "steam.steam_id";

/// Sync bookkeeping kept in Tauri state.
#[derive(Default)]
pub struct SyncState {
    running: AtomicBool,
    artwork_running: AtomicBool,
    last: Mutex<HashMap<SourceKind, SyncReport>>,
}

/// Steam settings resolved from the registry, the settings table and the vault.
fn steam_config(db: &Db, secrets: &Secrets) -> SteamConfig {
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

/// Every source the app knows. Launchers that are not installed are kept in
/// the list: they report `NotInstalled` and are skipped by the scan.
fn build_sources(db: &Db, secrets: &Secrets) -> Vec<Box<dyn Source>> {
    vec![Box::new(SteamSource::new(steam_config(db, secrets)))]
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

    let sources = build_sources(&db, &secrets);
    let mut reports = Vec::new();
    let mut jobs: Vec<ArtworkJob> = Vec::new();
    for scan in scan_all(&sources) {
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
    Ok(reports)
}

/// Downloads the artwork still missing, in the background, then refreshes the UI.
fn spawn_artwork_downloads(app: AppHandle, data_dir: PathBuf, jobs: Vec<ArtworkJob>) {
    if jobs.is_empty()
        || app
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
    let config = steam_config(&db, &secrets);
    let location = config.root.as_ref().map(|p| p.display().to_string());
    let source = SteamSource::new(config);
    let last = sync_state.last.lock().unwrap_or_else(|e| e.into_inner());
    let last_sync = last.get(&SourceKind::Steam).cloned();
    // A failed Web API call is only visible in the last sync report.
    let status = match &last_sync {
        Some(r) if matches!(r.status, SourceStatus::Error(_)) => r.status.clone(),
        _ => source.status(),
    };
    Ok(vec![SourceCard {
        kind: SourceKind::Steam,
        status,
        location,
        last_sync,
        syncing: sync_state.running.load(Ordering::SeqCst),
    }])
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
