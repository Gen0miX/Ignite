//! Applies a source scan to the database.

use std::collections::HashSet;

use rusqlite::Connection;
use serde::Serialize;

use crate::db::repo::{artwork, game_sources, games};
use crate::error::AppResult;
use crate::models::SourceKind;
use crate::sources::{ArtworkCandidate, ScanResult, SourceStatus};

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub kind: SourceKind,
    pub status: SourceStatus,
    pub added: u32,
    pub updated: u32,
    /// Games of this source no longer installed after this scan.
    pub uninstalled: u32,
    pub total: u32,
    pub warnings: Vec<String>,
    /// RFC 3339.
    pub finished_at: String,
}

/// Artwork still missing for a game after the sync.
#[derive(Debug, Clone)]
pub struct ArtworkJob {
    pub game_id: String,
    pub candidates: Vec<ArtworkCandidate>,
}

/// Writes one scan in a single transaction:
/// - known `(kind, external_id)` → source refreshed (user edits on the game are kept);
/// - new → game + source created;
/// - sources of this kind missing from the scan → marked not installed
///   (never deleted: a launcher can be offline or uninstalled temporarily).
pub fn apply_scan(
    conn: &mut Connection,
    scan: &ScanResult,
) -> AppResult<(SyncReport, Vec<ArtworkJob>)> {
    let mut report = SyncReport {
        kind: scan.kind,
        status: scan.status.clone(),
        added: 0,
        updated: 0,
        uninstalled: 0,
        total: scan.games.len() as u32,
        warnings: scan.warnings.clone(),
        finished_at: crate::db::now_rfc3339(),
    };
    let failed_completely = matches!(scan.status, SourceStatus::Error(_)) && scan.games.is_empty();
    if failed_completely {
        return Ok((report, Vec::new()));
    }

    let tx = conn.transaction()?;
    let mut jobs = Vec::new();
    let mut seen = HashSet::new();

    for g in &scan.games {
        seen.insert(g.external_id.clone());
        let existing = game_sources::find_by_external(&tx, scan.kind, &g.external_id)?;
        let mut data = game_sources::SourceData {
            external_id: Some(g.external_id.clone()),
            installed: g.installed,
            install_path: g.install_path.clone(),
            exe_path: None,
            size_bytes: g.size_bytes,
            launch_uri: g.launch_uri.clone(),
            process_matchers: None,
            source_playtime_minutes: g.playtime_minutes,
            source_last_played: g.last_played.clone(),
        };
        let game_id = match existing {
            Some(prev) => {
                // Keep what this scan could not provide (e.g. API offline this time).
                data.exe_path = prev.exe_path;
                data.process_matchers = prev.process_matchers;
                data.install_path = data.install_path.or(prev.install_path);
                data.size_bytes = data.size_bytes.or(prev.size_bytes);
                data.source_playtime_minutes = data
                    .source_playtime_minutes
                    .or(prev.source_playtime_minutes);
                data.source_last_played = data.source_last_played.or(prev.source_last_played);
                game_sources::upsert(&tx, &prev.game_id, scan.kind, &data)?;
                report.updated += 1;
                prev.game_id
            }
            None => {
                let game = games::insert(&tx, &g.title)?;
                game_sources::upsert(&tx, &game.id, scan.kind, &data)?;
                report.added += 1;
                game.id
            }
        };

        let have = artwork::for_game(&tx, &game_id)?;
        let missing: Vec<_> = g
            .artwork
            .iter()
            .filter(|c| {
                have.get(c.kind.as_str())
                    .is_none_or(|path| !std::path::Path::new(path).is_file())
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            jobs.push(ArtworkJob {
                game_id,
                candidates: missing,
            });
        }
    }

    for source in game_sources::list_by_kind(&tx, scan.kind)? {
        let still_there = source
            .external_id
            .as_ref()
            .is_some_and(|id| seen.contains(id));
        if !still_there && source.installed {
            game_sources::set_installed(&tx, &source.id, false)?;
            report.uninstalled += 1;
        }
    }

    tx.commit()?;
    Ok((report, jobs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games::GamePatch;
    use crate::db::Db;
    use crate::models::GameStatus;
    use crate::sources::DetectedGame;

    fn detected(id: &str, title: &str, installed: bool, minutes: Option<i64>) -> DetectedGame {
        DetectedGame {
            external_id: id.into(),
            title: title.into(),
            installed,
            launch_uri: Some(format!("steam://rungameid/{id}")),
            playtime_minutes: minutes,
            ..Default::default()
        }
    }

    fn scan(games: Vec<DetectedGame>, status: SourceStatus) -> ScanResult {
        ScanResult {
            games,
            ..ScanResult::empty(SourceKind::Steam, status)
        }
    }

    #[test]
    fn first_sync_creates_games_then_updates_them() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let first = scan(
            vec![
                detected("10", "Game A", true, Some(60)),
                detected("20", "Game B", false, None),
            ],
            SourceStatus::Connected,
        );
        let (report, _) = apply_scan(&mut conn, &first).unwrap();
        assert_eq!((report.added, report.updated, report.total), (2, 0, 2));

        // The user edits a game; a second sync must keep the edits.
        let a = game_sources::find_by_external(&conn, SourceKind::Steam, "10")
            .unwrap()
            .unwrap();
        let patch = GamePatch {
            title: Some("Mon jeu A".into()),
            status: Some(GameStatus::Done),
            ..Default::default()
        };
        games::update(&conn, &a.game_id, &patch).unwrap();

        let second = scan(
            vec![
                detected("10", "Game A", true, Some(90)),
                detected("20", "Game B", false, None),
            ],
            SourceStatus::Connected,
        );
        let (report, _) = apply_scan(&mut conn, &second).unwrap();
        assert_eq!((report.added, report.updated), (0, 2));
        let game = games::get(&conn, &a.game_id).unwrap();
        assert_eq!(game.title, "Mon jeu A");
        assert_eq!(game.status, GameStatus::Done);
        let src = game_sources::get(&conn, &a.id).unwrap();
        assert_eq!(src.source_playtime_minutes, Some(90));
        assert_eq!(games::list(&conn, true).unwrap().len(), 2);
    }

    #[test]
    fn missing_games_are_marked_uninstalled_not_deleted() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        apply_scan(
            &mut conn,
            &scan(
                vec![detected("10", "A", true, None)],
                SourceStatus::Connected,
            ),
        )
        .unwrap();
        let (report, _) = apply_scan(&mut conn, &scan(vec![], SourceStatus::NeedsAuth)).unwrap();
        assert_eq!(report.uninstalled, 1);
        let src = game_sources::find_by_external(&conn, SourceKind::Steam, "10")
            .unwrap()
            .unwrap();
        assert!(!src.installed);
        assert_eq!(games::list(&conn, true).unwrap().len(), 1);
    }

    #[test]
    fn a_failed_scan_changes_nothing() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        apply_scan(
            &mut conn,
            &scan(
                vec![detected("10", "A", true, Some(5))],
                SourceStatus::Connected,
            ),
        )
        .unwrap();
        let (report, jobs) =
            apply_scan(&mut conn, &scan(vec![], SourceStatus::Error("boom".into()))).unwrap();
        assert_eq!(report.uninstalled, 0);
        assert!(jobs.is_empty());
        let src = game_sources::find_by_external(&conn, SourceKind::Steam, "10")
            .unwrap()
            .unwrap();
        assert!(src.installed);
    }

    #[test]
    fn keeps_previous_playtime_when_the_scan_has_none() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        apply_scan(
            &mut conn,
            &scan(
                vec![detected("10", "A", true, Some(300))],
                SourceStatus::Connected,
            ),
        )
        .unwrap();
        apply_scan(
            &mut conn,
            &scan(
                vec![detected("10", "A", true, None)],
                SourceStatus::Error("api".into()),
            ),
        )
        .unwrap();
        let src = game_sources::find_by_external(&conn, SourceKind::Steam, "10")
            .unwrap()
            .unwrap();
        assert_eq!(src.source_playtime_minutes, Some(300));
    }
}
