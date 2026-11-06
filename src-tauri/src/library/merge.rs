//! Multi-platform duplicates (SPEC § 2 "Doublons"): one game, several sources.

use std::collections::BTreeMap;

use rusqlite::{params, Connection};

use crate::db::repo::{game_sources, games};
use crate::db::{new_id, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::GameStatus;

/// Edition words dropped when comparing titles across platforms.
const EDITION_WORDS: &[&str] = &[
    "goty",
    "remastered",
    "remaster",
    "definitive",
    "enhanced",
    "complete",
    "deluxe",
    "ultimate",
    "anniversary",
    "director's",
    "directors",
    "cut",
    "edition",
    "game of the year",
];

/// Key used to detect the same game on two platforms: lowercase, accents and
/// trademark signs removed, punctuation dropped, edition words removed.
/// "The Witcher 3: Wild Hunt — Remastered" and "The Witcher® 3: Wild Hunt"
/// give the same key.
pub fn match_key(title: &str) -> String {
    let folded = games::sort_title(title);
    let cleaned: String = folded
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '\'' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let mut text = format!(
        " {} ",
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    for word in EDITION_WORDS {
        text = text.replace(&format!(" {word} "), " ");
    }
    let key = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if key.is_empty() {
        folded
    } else {
        key
    }
}

/// Groups of visible games that look like the same title on different platforms.
pub fn duplicate_groups(conn: &Connection) -> AppResult<Vec<Vec<String>>> {
    let sources = game_sources::list_all(conn)?;
    let mut kinds: BTreeMap<String, Vec<crate::models::SourceKind>> = BTreeMap::new();
    for s in &sources {
        kinds.entry(s.game_id.clone()).or_default().push(s.kind);
    }
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for g in games::list(conn, false)? {
        groups.entry(match_key(&g.title)).or_default().push(g.id);
    }
    Ok(groups
        .into_values()
        .filter(|ids| {
            if ids.len() < 2 {
                return false;
            }
            // Same platform twice (two Steam apps) is not a cross-platform duplicate.
            let mut seen = Vec::new();
            for id in ids {
                for k in kinds.get(id).into_iter().flatten() {
                    if seen.contains(k) {
                        return false;
                    }
                    seen.push(*k);
                }
            }
            true
        })
        .collect())
}

/// Most advanced status wins when merging (done > playing > dropped > todo).
fn merged_status(a: GameStatus, b: GameStatus) -> GameStatus {
    let rank = |s| match s {
        GameStatus::Done => 3,
        GameStatus::Playing => 2,
        GameStatus::Dropped => 1,
        GameStatus::Todo => 0,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// Moves everything of `others` into `target` (sources, sessions, goals,
/// tags, collections, missing artwork) and deletes them, in one transaction.
pub fn merge(conn: &mut Connection, target: &str, others: &[String]) -> AppResult<()> {
    if others.is_empty() || others.iter().any(|o| o == target) {
        return Err(AppError::Invalid("nothing to merge".into()));
    }
    let tx = conn.transaction()?;
    let mut keep = games::get(&tx, target)?;
    for other_id in others {
        let other = games::get(&tx, other_id)?;
        for sql in [
            "UPDATE game_sources SET game_id = ?1 WHERE game_id = ?2",
            "UPDATE play_sessions SET game_id = ?1 WHERE game_id = ?2",
            "UPDATE goals SET game_id = ?1, position = position + 1000 WHERE game_id = ?2",
            "INSERT OR IGNORE INTO game_tags (game_id, tag_id) SELECT ?1, tag_id FROM game_tags WHERE game_id = ?2",
            "INSERT OR IGNORE INTO collection_games (collection_id, game_id) SELECT collection_id, ?1 FROM collection_games WHERE game_id = ?2",
            "INSERT OR IGNORE INTO artwork (game_id, kind, local_path, provider, source_url) \
               SELECT ?1, kind, local_path, provider, source_url FROM artwork WHERE game_id = ?2",
        ] {
            tx.execute(sql, params![target, other_id])?;
        }
        keep.status = merged_status(keep.status, other.status);
        keep.favorite |= other.favorite;
        keep.notes = match (keep.notes.take(), other.notes) {
            (Some(a), Some(b)) if a != b => Some(format!("{a}\n\n{b}")),
            (a, b) => a.or(b),
        };
        keep.igdb_id = keep.igdb_id.or(other.igdb_id);
        keep.steamgriddb_id = keep.steamgriddb_id.or(other.steamgriddb_id);
        tx.execute("DELETE FROM games WHERE id = ?1", [other_id])?;
    }
    tx.execute(
        "UPDATE games SET status = ?2, favorite = ?3, notes = ?4, igdb_id = ?5, steamgriddb_id = ?6, \
         updated_at = ?7 WHERE id = ?1",
        params![
            keep.id,
            keep.status,
            keep.favorite,
            keep.notes,
            keep.igdb_id,
            keep.steamgriddb_id,
            now_rfc3339()
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// Detaches one source into a new game of its own (undo a merge). Sessions
/// recorded for that source follow it. Returns the new game id.
pub fn split(conn: &mut Connection, source_id: &str) -> AppResult<String> {
    let tx = conn.transaction()?;
    let source = game_sources::get(&tx, source_id)?;
    let siblings = game_sources::list_for_game(&tx, &source.game_id)?;
    if siblings.len() < 2 {
        return Err(AppError::Invalid("the game has a single source".into()));
    }
    let original = games::get(&tx, &source.game_id)?;
    let id = new_id();
    let now = now_rfc3339();
    tx.execute(
        "INSERT INTO games (id, title, sort_title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, original.title, original.sort_title, now],
    )?;
    tx.execute(
        "UPDATE game_sources SET game_id = ?1 WHERE id = ?2",
        params![id, source_id],
    )?;
    tx.execute(
        "UPDATE play_sessions SET game_id = ?1 WHERE game_source_id = ?2",
        params![id, source_id],
    )?;
    if original.preferred_source_id.as_deref() == Some(source_id) {
        tx.execute(
            "UPDATE games SET preferred_source_id = NULL WHERE id = ?1",
            [&original.id],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::repo::sessions::{self, PlaySession, SessionOrigin};
    use crate::db::repo::{goals, tags};
    use crate::db::Db;
    use crate::library::query;
    use crate::models::SourceKind;

    #[test]
    fn match_keys_ignore_editions_and_symbols() {
        let same = [
            "The Witcher 3: Wild Hunt — Remastered",
            "The Witcher® 3: Wild Hunt",
            "THE WITCHER 3 - WILD HUNT GOTY",
            "The Witcher 3: Wild Hunt - Game of the Year Edition",
        ];
        for t in same {
            assert_eq!(match_key(t), "witcher 3 wild hunt", "{t}");
        }
        assert_eq!(
            match_key("Ultima™ IV: Quest of the Avatar"),
            "ultima iv quest of the avatar"
        );
        assert_ne!(match_key("The Witcher 2"), match_key("The Witcher 3"));
        assert_eq!(match_key("Edition"), "edition");
    }

    fn add(
        conn: &Connection,
        title: &str,
        kind: SourceKind,
        ext: &str,
        minutes: Option<i64>,
    ) -> (String, String) {
        let g = games::insert(conn, title).unwrap();
        let s = game_sources::upsert(
            conn,
            &g.id,
            kind,
            &SourceData {
                external_id: Some(ext.into()),
                installed: true,
                source_playtime_minutes: minutes,
                ..Default::default()
            },
        )
        .unwrap();
        (g.id, s.id)
    }

    #[test]
    fn finds_cross_platform_duplicates_only() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        add(&conn, "Cyberpunk 2077", SourceKind::Steam, "1091500", None);
        add(&conn, "Cyberpunk 2077", SourceKind::Gog, "1423049311", None);
        add(&conn, "Prey", SourceKind::Steam, "480490", None);
        add(&conn, "Prey", SourceKind::Steam, "3970", None); // two Steam apps: not merged
        add(&conn, "Hades", SourceKind::Steam, "1145360", None);
        let groups = duplicate_groups(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    /// J3 "done when": a GOG game also owned on Steam has one sheet with
    /// both times broken down.
    #[test]
    fn merged_game_has_one_sheet_and_both_times() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let (steam_game, steam_src) = add(
            &conn,
            "Cyberpunk 2077",
            SourceKind::Steam,
            "1091500",
            Some(600),
        );
        let (gog_game, gog_src) = add(
            &conn,
            "Cyberpunk 2077",
            SourceKind::Gog,
            "1423049311",
            Some(1200),
        );
        games::update(
            &conn,
            &gog_game,
            &games::GamePatch {
                status: Some(GameStatus::Done),
                notes: Some(Some("Fini".into())),
                ..Default::default()
            },
        )
        .unwrap();
        tags::add_to_game(&conn, &gog_game, "RPG").unwrap();
        goals::add(&conn, &gog_game, "Toutes les fins").unwrap();
        sessions::save(
            &conn,
            &PlaySession {
                id: "s1".into(),
                game_id: gog_game.clone(),
                game_source_id: Some(gog_src.clone()),
                started_at: Some("2026-11-01T10:00:00Z".into()),
                ended_at: Some("2026-11-01T11:00:00Z".into()),
                duration_seconds: 3600,
                idle_seconds: 0,
                origin: SessionOrigin::Auto,
                note: None,
                rating: None,
            },
        )
        .unwrap();

        merge(&mut conn, &steam_game, std::slice::from_ref(&gog_game)).unwrap();

        let list = query::summaries(&conn).unwrap();
        assert_eq!(list.len(), 1);
        let g = &list[0];
        assert_eq!(g.sources, vec![SourceKind::Steam, SourceKind::Gog]);
        // Both platform counters, the GOG auto session not added on top.
        assert_eq!(g.playtime_minutes, 1800);
        assert_eq!(g.status, GameStatus::Done);
        let kept = games::get(&conn, &steam_game).unwrap();
        assert_eq!(kept.notes.as_deref(), Some("Fini"));
        assert_eq!(tags::for_game(&conn, &steam_game).unwrap().len(), 1);
        assert_eq!(goals::list(&conn, &steam_game).unwrap().len(), 1);
        assert_eq!(
            sessions::list_for_game(&conn, &steam_game).unwrap().len(),
            1
        );
        assert!(games::find(&conn, &gog_game).unwrap().is_none());

        // A later sync keeps the GOG source attached to the merged game.
        let again = game_sources::upsert(
            &conn,
            "ignored-for-existing",
            SourceKind::Gog,
            &SourceData {
                external_id: Some("1423049311".into()),
                installed: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(again.game_id, steam_game);

        // Split it back out: the GOG source and its session move to a new game.
        let new_id = split(&mut conn, &gog_src).unwrap();
        assert_eq!(game_sources::get(&conn, &gog_src).unwrap().game_id, new_id);
        assert_eq!(sessions::list_for_game(&conn, &new_id).unwrap().len(), 1);
        assert_eq!(query::summaries(&conn).unwrap().len(), 2);
        assert!(split(&mut conn, &steam_src).is_err()); // last source of its game
    }

    #[test]
    fn rejects_empty_or_self_merges() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let (a, _) = add(&conn, "A", SourceKind::Steam, "1", None);
        assert!(merge(&mut conn, &a, &[]).is_err());
        assert!(merge(&mut conn, &a, std::slice::from_ref(&a)).is_err());
    }
}
