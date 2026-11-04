//! Cached achievements per game source.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::{new_id, now_rfc3339};
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq)]
pub struct AchievementRow {
    pub api_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    /// RFC 3339 when unlocked.
    pub unlocked_at: Option<String>,
    pub global_percent: Option<f64>,
}

/// Replaces the cache of one source. `list = None` records "no achievements".
pub fn replace_for_source(
    conn: &mut Connection,
    source_id: &str,
    list: Option<&[AchievementRow]>,
) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM achievements WHERE game_source_id = ?1",
        [source_id],
    )?;
    for a in list.unwrap_or_default() {
        tx.execute(
            "INSERT INTO achievements (id, game_source_id, api_name, name, description, icon_url, \
               unlocked_at, global_percent) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                new_id(),
                source_id,
                a.api_name,
                a.name,
                a.description,
                a.icon_url,
                a.unlocked_at,
                a.global_percent
            ],
        )?;
    }
    let total = list.map(|l| l.len() as i64).unwrap_or(0);
    tx.execute(
        "UPDATE game_sources SET achievements_synced_at = ?2, achievements_total = ?3 WHERE id = ?1",
        params![source_id, now_rfc3339(), total],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn list_for_source(conn: &Connection, source_id: &str) -> AppResult<Vec<AchievementRow>> {
    let mut stmt = conn.prepare(
        "SELECT api_name, name, description, icon_url, unlocked_at, global_percent \
         FROM achievements WHERE game_source_id = ?1 ORDER BY api_name",
    )?;
    let rows = stmt.query_map([source_id], |r| {
        Ok(AchievementRow {
            api_name: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            icon_url: r.get(3)?,
            unlocked_at: r.get(4)?,
            global_percent: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// `(synced_at, total)` of a source's cache, if it was ever fetched.
pub fn sync_state(conn: &Connection, source_id: &str) -> AppResult<Option<(String, i64)>> {
    Ok(conn
        .query_row(
            "SELECT achievements_synced_at, achievements_total FROM game_sources \
             WHERE id = ?1 AND achievements_synced_at IS NOT NULL",
            [source_id],
            |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0))),
        )
        .optional()?)
}

/// `game_id → (unlocked, total)` over all sources with achievements.
pub fn progress_by_game(conn: &Connection) -> AppResult<HashMap<String, (u32, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT s.game_id, COUNT(a.id), SUM(a.unlocked_at IS NOT NULL) \
         FROM game_sources s JOIN achievements a ON a.game_source_id = s.id GROUP BY s.game_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            (r.get::<_, i64>(2)? as u32, r.get::<_, i64>(1)? as u32),
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::{self, SourceData};
    use crate::db::repo::games;
    use crate::db::Db;
    use crate::models::SourceKind;

    fn row(name: &str, unlocked: bool) -> AchievementRow {
        AchievementRow {
            api_name: name.into(),
            name: Some(name.into()),
            description: None,
            icon_url: None,
            unlocked_at: unlocked.then(|| "2026-01-01T00:00:00Z".into()),
            global_percent: Some(12.5),
        }
    }

    #[test]
    fn replace_list_and_progress() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        let s = game_sources::upsert(
            &conn,
            &g.id,
            SourceKind::Steam,
            &SourceData {
                external_id: Some("1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(sync_state(&conn, &s.id).unwrap(), None);

        replace_for_source(
            &mut conn,
            &s.id,
            Some(&[row("A", true), row("B", false), row("C", true)]),
        )
        .unwrap();
        assert_eq!(list_for_source(&conn, &s.id).unwrap().len(), 3);
        assert_eq!(progress_by_game(&conn).unwrap()[&g.id], (2, 3));
        assert_eq!(sync_state(&conn, &s.id).unwrap().unwrap().1, 3);

        replace_for_source(&mut conn, &s.id, None).unwrap();
        assert!(list_for_source(&conn, &s.id).unwrap().is_empty());
        assert_eq!(sync_state(&conn, &s.id).unwrap().unwrap().1, 0);
        assert!(progress_by_game(&conn).unwrap().is_empty());
    }
}
