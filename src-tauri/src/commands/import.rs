//! Import wizard commands: scan → preview → commit.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::sources::{build_sources, run_sync, set_enabled};
use super::CommandResult;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::library::import::{self, ImportCandidate, ImportDecision};
use crate::library::merge;
use crate::models::SourceKind;
use crate::secrets::Secrets;
use crate::sources::{scan_all, SourceStatus};

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScannedSource {
    pub kind: SourceKind,
    pub status: SourceStatus,
    pub detected: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, specta::Type)]
pub struct ImportScan {
    pub sources: Vec<ScannedSource>,
    pub candidates: Vec<ImportCandidate>,
}

/// Scans the given sources (enabled or not) without writing anything.
#[tauri::command]
#[specta::specta]
pub async fn import_scan(app: AppHandle, kinds: Vec<SourceKind>) -> CommandResult<ImportScan> {
    let result = tauri::async_runtime::spawn_blocking(move || -> CommandResult<ImportScan> {
        let db = app.state::<Db>();
        let secrets = app.state::<Secrets>();
        let sources: Vec<_> = build_sources(&db, &secrets, false)
            .into_iter()
            .filter(|s| kinds.contains(&s.kind()))
            .collect();
        let scans = scan_all(&sources);
        let candidates = import::candidates(&db.conn(), &scans)?;
        Ok(ImportScan {
            sources: scans
                .iter()
                .map(|s| ScannedSource {
                    kind: s.kind,
                    status: s.status.clone(),
                    detected: s.games.len() as u32,
                    warnings: s.warnings.clone(),
                })
                .collect(),
            candidates,
        })
    })
    .await
    .map_err(|e| AppError::Invalid(format!("scan task failed: {e}")))?;
    result
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommit {
    /// Sources to turn on (their games are then kept in sync).
    pub enable: Vec<SourceKind>,
    pub decisions: Vec<ImportDecision>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: u32,
    pub merged: u32,
    pub skipped: u32,
    /// Imported games still without a cover (to pick one from SteamGridDB).
    pub without_cover: Vec<String>,
}

/// Records the choices, enables the sources, syncs, then performs merges.
#[tauri::command]
#[specta::specta]
pub async fn import_commit(app: AppHandle, commit: ImportCommit) -> CommandResult<ImportReport> {
    let result = tauri::async_runtime::spawn_blocking(move || -> CommandResult<ImportReport> {
        let db = app.state::<Db>();
        let skipped = commit
            .decisions
            .iter()
            .filter(|d| matches!(d.action, import::ImportAction::Skip))
            .count() as u32;
        let merges = import::record_decisions(&db.conn(), &commit.decisions)?;
        for kind in &commit.enable {
            set_enabled(&db, *kind, true)?;
        }
        let reports = run_sync(&app)?;
        let added = reports
            .iter()
            .filter(|r| commit.enable.contains(&r.kind))
            .map(|r| r.added)
            .sum();

        let mut merged = 0;
        for (key, target) in merges {
            let Some(game) = import::game_for_key(&db.conn(), &key)? else {
                continue;
            };
            if game != target {
                match merge::merge(&mut db.conn(), &target, std::slice::from_ref(&game)) {
                    Ok(()) => merged += 1,
                    Err(e) => tracing::warn!(%key, error = %e, "merge after import failed"),
                }
            }
        }

        let without_cover = games_without_cover(&db, &commit.enable)?;
        Ok(ImportReport {
            added,
            merged,
            skipped,
            without_cover,
        })
    })
    .await
    .map_err(|e| AppError::Invalid(format!("import task failed: {e}")))?;
    result
}

/// Visible games of these sources that have neither a cover nor a header.
fn games_without_cover(db: &Db, kinds: &[SourceKind]) -> AppResult<Vec<String>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT g.id FROM games g JOIN game_sources s ON s.game_id = g.id          WHERE s.kind IN (SELECT value FROM json_each(?1)) AND g.hidden = 0          AND NOT EXISTS (SELECT 1 FROM artwork a WHERE a.game_id = g.id AND a.kind IN ('cover', 'header'))          GROUP BY g.id",
    )?;
    let kinds = serde_json::to_string(kinds)?;
    let ids = stmt
        .query_map([kinds], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
