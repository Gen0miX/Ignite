//! Import wizard (SPEC § 3.3): preview what each source detected, flag
//! duplicates and non-games, then import the user's selection.
//!
//! Entries the user unticks are remembered (`import.ignored`) so later
//! automatic syncs do not bring them back.

use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::repo::{game_sources, games, settings};
use crate::error::AppResult;
use crate::library::merge::match_key;
use crate::library::non_games::is_probably_not_a_game;
use crate::models::SourceKind;
use crate::sources::{DetectedGame, ScanResult};

const IGNORED_KEY: &str = "import.ignored";

/// `kind:external_id`, stable across scans.
pub fn candidate_key(kind: SourceKind, external_id: &str) -> String {
    format!("{}:{external_id}", kind.as_str())
}

pub fn ignored(conn: &Connection) -> AppResult<BTreeSet<String>> {
    Ok(settings::get::<BTreeSet<String>>(conn, IGNORED_KEY)?.unwrap_or_default())
}

pub fn set_ignored(conn: &Connection, keys: &BTreeSet<String>) -> AppResult<()> {
    settings::set(conn, IGNORED_KEY, keys)
}

/// Removes ignored entries from a scan before it is written.
pub fn without_ignored(mut scan: ScanResult, ignored: &BTreeSet<String>) -> ScanResult {
    let kind = scan.kind;
    scan.games
        .retain(|g| !ignored.contains(&candidate_key(kind, &g.external_id)));
    scan
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CandidateState {
    /// Not in the library yet.
    New,
    /// This exact source is already imported.
    Imported {
        #[serde(rename = "gameId")]
        game_id: String,
    },
    /// Looks like a game already in the library on another platform.
    Duplicate {
        #[serde(rename = "gameId")]
        game_id: String,
        title: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CandidateNote {
    /// Playtest, beta, demo, DLC variant...
    NotAGame,
    /// Unticked in a previous import.
    PreviouslyIgnored,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub key: String,
    pub source: SourceKind,
    pub external_id: String,
    pub title: String,
    pub installed: bool,
    pub install_path: Option<String>,
    pub size_bytes: Option<f64>,
    pub playtime_minutes: Option<u32>,
    pub achievements: Option<u32>,
    pub state: CandidateState,
    pub note: Option<CandidateNote>,
    /// Ticked by default in the wizard.
    pub selected: bool,
}

fn candidate(
    conn: &Connection,
    kind: SourceKind,
    g: &DetectedGame,
    by_key: &HashMap<String, (String, String, Vec<SourceKind>)>,
    ignored: &BTreeSet<String>,
) -> AppResult<ImportCandidate> {
    let key = candidate_key(kind, &g.external_id);
    let state = match game_sources::find_by_external(conn, kind, &g.external_id)? {
        Some(src) => CandidateState::Imported {
            game_id: src.game_id,
        },
        None => match by_key.get(&match_key(&g.title)) {
            Some((id, title, kinds)) if !kinds.contains(&kind) => CandidateState::Duplicate {
                game_id: id.clone(),
                title: title.clone(),
            },
            _ => CandidateState::New,
        },
    };
    let note = if ignored.contains(&key) {
        Some(CandidateNote::PreviouslyIgnored)
    } else if g.hide_on_import || is_probably_not_a_game(&g.title) {
        Some(CandidateNote::NotAGame)
    } else {
        None
    };
    let selected = match (&state, note) {
        (CandidateState::Imported { .. }, _) => true,
        (_, Some(_)) => false,
        _ => true,
    };
    Ok(ImportCandidate {
        key,
        source: kind,
        external_id: g.external_id.clone(),
        title: g.title.clone(),
        installed: g.installed,
        install_path: g.install_path.clone(),
        size_bytes: g.size_bytes.map(|b| b as f64),
        playtime_minutes: g
            .playtime_minutes
            .map(|m| m.clamp(0, u32::MAX as i64) as u32),
        achievements: g.achievements.as_ref().map(|a| a.len() as u32),
        state,
        note,
        selected,
    })
}

/// Builds the wizard table from fresh scans.
pub fn candidates(conn: &Connection, scans: &[ScanResult]) -> AppResult<Vec<ImportCandidate>> {
    let ignored = ignored(conn)?;
    let mut kinds: HashMap<String, Vec<SourceKind>> = HashMap::new();
    for s in game_sources::list_all(conn)? {
        kinds.entry(s.game_id).or_default().push(s.kind);
    }
    let mut by_key = HashMap::new();
    for g in games::list(conn, true)? {
        let k = kinds.remove(&g.id).unwrap_or_default();
        by_key
            .entry(match_key(&g.title))
            .or_insert((g.id, g.title, k));
    }
    let mut list = Vec::new();
    for scan in scans {
        for g in &scan.games {
            list.push(candidate(conn, scan.kind, g, &by_key, &ignored)?);
        }
    }
    list.sort_by(|a, b| {
        let rank = |c: &ImportCandidate| match c.state {
            CandidateState::Duplicate { .. } => 0,
            CandidateState::New => 1,
            CandidateState::Imported { .. } => 2,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| games::sort_title(&a.title).cmp(&games::sort_title(&b.title)))
    });
    Ok(list)
}

/// What the user decided for one candidate.
#[derive(Debug, Clone, PartialEq, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ImportAction {
    Import,
    /// Import and merge into an existing game.
    Merge {
        #[serde(rename = "gameId")]
        game_id: String,
    },
    Skip,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ImportDecision {
    pub key: String,
    pub action: ImportAction,
}

/// Updates the ignored list from the decisions: skipped entries are added,
/// imported ones removed. Returns the merges to perform after the sync.
pub fn record_decisions(
    conn: &Connection,
    decisions: &[ImportDecision],
) -> AppResult<Vec<(String, String)>> {
    let mut ignored = ignored(conn)?;
    let mut merges = Vec::new();
    for d in decisions {
        match &d.action {
            ImportAction::Skip => {
                ignored.insert(d.key.clone());
            }
            ImportAction::Import => {
                ignored.remove(&d.key);
            }
            ImportAction::Merge { game_id } => {
                ignored.remove(&d.key);
                merges.push((d.key.clone(), game_id.clone()));
            }
        }
    }
    set_ignored(conn, &ignored)?;
    Ok(merges)
}

/// Game created for a candidate key after the sync (`kind:external_id`).
pub fn game_for_key(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let Some((kind, ext)) = key.split_once(':') else {
        return Ok(None);
    };
    let Ok(kind) = kind.parse::<SourceKind>() else {
        return Ok(None);
    };
    Ok(game_sources::find_by_external(conn, kind, ext)?.map(|s| s.game_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::Db;
    use crate::sources::SourceStatus;

    fn detected(id: &str, title: &str) -> DetectedGame {
        DetectedGame {
            external_id: id.into(),
            title: title.into(),
            installed: true,
            ..Default::default()
        }
    }

    fn scan(kind: SourceKind, games: Vec<DetectedGame>) -> ScanResult {
        ScanResult {
            games,
            ..ScanResult::empty(kind, SourceStatus::Connected)
        }
    }

    #[test]
    fn classifies_new_imported_duplicate_and_non_games() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Cyberpunk 2077").unwrap();
        game_sources::upsert(
            &conn,
            &g.id,
            SourceKind::Steam,
            &SourceData {
                external_id: Some("1091500".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let mut dlc = detected("4", "dlc_13_a");
        dlc.hide_on_import = true;

        let gog = scan(
            SourceKind::Gog,
            vec![
                detected("1", "Cyberpunk 2077"),
                detected("2", "Teenagent"),
                detected("3", "Arkheron Playtest"),
                dlc,
            ],
        );
        let steam = scan(
            SourceKind::Steam,
            vec![detected("1091500", "Cyberpunk 2077")],
        );
        let list = candidates(&conn, &[gog, steam]).unwrap();

        let find = |key: &str| list.iter().find(|c| c.key == key).unwrap();
        assert_eq!(
            find("gog:1").state,
            CandidateState::Duplicate {
                game_id: g.id.clone(),
                title: "Cyberpunk 2077".into()
            }
        );
        assert!(find("gog:1").selected);
        assert_eq!(find("gog:2").state, CandidateState::New);
        assert_eq!(find("gog:3").note, Some(CandidateNote::NotAGame));
        assert!(!find("gog:3").selected);
        assert!(!find("gog:4").selected);
        assert_eq!(
            find("steam:1091500").state,
            CandidateState::Imported {
                game_id: g.id.clone()
            }
        );
        // Duplicates first, already imported last.
        assert_eq!(list[0].key, "gog:1");
        assert_eq!(list.last().unwrap().key, "steam:1091500");
    }

    #[test]
    fn skipped_entries_are_remembered_and_filtered_from_syncs() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let merges = record_decisions(
            &conn,
            &[
                ImportDecision {
                    key: "gog:3".into(),
                    action: ImportAction::Skip,
                },
                ImportDecision {
                    key: "gog:1".into(),
                    action: ImportAction::Merge {
                        game_id: "g".into(),
                    },
                },
            ],
        )
        .unwrap();
        assert_eq!(merges, vec![("gog:1".to_string(), "g".to_string())]);
        let ignored = ignored(&conn).unwrap();
        assert!(ignored.contains("gog:3"));

        let filtered = without_ignored(
            scan(
                SourceKind::Gog,
                vec![detected("1", "A"), detected("3", "B")],
            ),
            &ignored,
        );
        assert_eq!(filtered.games.len(), 1);

        // Imported later from the wizard: no longer ignored.
        record_decisions(
            &conn,
            &[ImportDecision {
                key: "gog:3".into(),
                action: ImportAction::Import,
            }],
        )
        .unwrap();
        assert!(super::ignored(&conn).unwrap().is_empty());
        let list = candidates(&conn, &[scan(SourceKind::Gog, vec![detected("3", "B")])]).unwrap();
        assert_eq!(list[0].note, None);
    }

    #[test]
    fn states_serialize_with_camel_case_fields() {
        let json = serde_json::to_string(&CandidateState::Imported {
            game_id: "g".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"imported","gameId":"g"}"#);
    }

    #[test]
    fn decisions_deserialize_from_the_frontend_shape() {
        let d: Vec<ImportDecision> = serde_json::from_str(
            r#"[{"key":"gog:1","action":{"type":"merge","gameId":"g1"}},{"key":"gog:2","action":{"type":"skip"}},{"key":"gog:3","action":{"type":"import"}}]"#,
        )
        .unwrap();
        assert_eq!(
            d[0].action,
            ImportAction::Merge {
                game_id: "g1".into()
            }
        );
        assert_eq!(d[1].action, ImportAction::Skip);
    }

    #[test]
    fn finds_the_game_of_a_key() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Teenagent").unwrap();
        game_sources::upsert(
            &conn,
            &g.id,
            SourceKind::Gog,
            &SourceData {
                external_id: Some("2".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(game_for_key(&conn, "gog:2").unwrap(), Some(g.id));
        assert_eq!(game_for_key(&conn, "gog:9").unwrap(), None);
        assert_eq!(game_for_key(&conn, "bogus").unwrap(), None);
    }
}
