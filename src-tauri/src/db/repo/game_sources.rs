use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::{new_id, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::SourceKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSource {
    pub id: String,
    pub game_id: String,
    pub kind: SourceKind,
    pub external_id: Option<String>,
    pub installed: bool,
    pub install_path: Option<String>,
    pub exe_path: Option<String>,
    pub size_bytes: Option<i64>,
    pub launch_uri: Option<String>,
    /// Raw JSON array of process matchers (typed in the tracker module, J2).
    pub process_matchers: Option<String>,
    pub source_playtime_minutes: Option<i64>,
    pub source_last_played: Option<String>,
    pub last_synced_at: Option<String>,
}

/// Data reported by a source scan. Upserted on `(kind, external_id)`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceData {
    pub external_id: Option<String>,
    pub installed: bool,
    pub install_path: Option<String>,
    pub exe_path: Option<String>,
    pub size_bytes: Option<i64>,
    pub launch_uri: Option<String>,
    pub process_matchers: Option<String>,
    pub source_playtime_minutes: Option<i64>,
    pub source_last_played: Option<String>,
}

const COLUMNS: &str = "id, game_id, kind, external_id, installed, install_path, exe_path, \
     size_bytes, launch_uri, process_matchers, source_playtime_minutes, source_last_played, \
     last_synced_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<GameSource> {
    Ok(GameSource {
        id: r.get(0)?,
        game_id: r.get(1)?,
        kind: r.get(2)?,
        external_id: r.get(3)?,
        installed: r.get(4)?,
        install_path: r.get(5)?,
        exe_path: r.get(6)?,
        size_bytes: r.get(7)?,
        launch_uri: r.get(8)?,
        process_matchers: r.get(9)?,
        source_playtime_minutes: r.get(10)?,
        source_last_played: r.get(11)?,
        last_synced_at: r.get(12)?,
    })
}

pub fn find_by_external(
    conn: &Connection,
    kind: SourceKind,
    external_id: &str,
) -> AppResult<Option<GameSource>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM game_sources WHERE kind = ?1 AND external_id = ?2"),
            params![kind, external_id],
            from_row,
        )
        .optional()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<GameSource> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM game_sources WHERE id = ?1"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("game source {id}")))
}

/// Inserts a source for `game_id`, or refreshes the existing row with the same
/// `(kind, external_id)`. An existing row keeps its `game_id` (merges are explicit).
pub fn upsert(
    conn: &Connection,
    game_id: &str,
    kind: SourceKind,
    data: &SourceData,
) -> AppResult<GameSource> {
    let existing = match &data.external_id {
        Some(ext) => find_by_external(conn, kind, ext)?,
        None => None,
    };
    let id = existing.map(|s| s.id).unwrap_or_else(new_id);
    conn.execute(
        "INSERT INTO game_sources (id, game_id, kind, external_id, installed, install_path, \
           exe_path, size_bytes, launch_uri, process_matchers, source_playtime_minutes, \
           source_last_played, last_synced_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
         ON CONFLICT(id) DO UPDATE SET installed = excluded.installed, \
           install_path = excluded.install_path, exe_path = excluded.exe_path, \
           size_bytes = excluded.size_bytes, launch_uri = excluded.launch_uri, \
           process_matchers = excluded.process_matchers, \
           source_playtime_minutes = excluded.source_playtime_minutes, \
           source_last_played = excluded.source_last_played, \
           last_synced_at = excluded.last_synced_at",
        params![
            id,
            game_id,
            kind,
            data.external_id,
            data.installed,
            data.install_path,
            data.exe_path,
            data.size_bytes,
            data.launch_uri,
            data.process_matchers,
            data.source_playtime_minutes,
            data.source_last_played,
            now_rfc3339()
        ],
    )?;
    get(conn, &id)
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<GameSource>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM game_sources ORDER BY kind, id"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn list_by_kind(conn: &Connection, kind: SourceKind) -> AppResult<Vec<GameSource>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM game_sources WHERE kind = ?1 ORDER BY id"
    ))?;
    let rows = stmt.query_map([kind], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn set_installed(conn: &Connection, id: &str, installed: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE game_sources SET installed = ?2 WHERE id = ?1",
        params![id, installed],
    )?;
    Ok(())
}

pub fn list_for_game(conn: &Connection, game_id: &str) -> AppResult<Vec<GameSource>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM game_sources WHERE game_id = ?1 ORDER BY kind, id"
    ))?;
    let rows = stmt.query_map([game_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    fn steam(appid: &str, minutes: i64) -> SourceData {
        SourceData {
            external_id: Some(appid.into()),
            installed: true,
            launch_uri: Some(format!("steam://rungameid/{appid}")),
            source_playtime_minutes: Some(minutes),
            ..Default::default()
        }
    }

    #[test]
    fn upsert_inserts_then_updates_same_external_id() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let game = games::insert(&conn, "DARK SOULS III").unwrap();
        let first = upsert(&conn, &game.id, SourceKind::Steam, &steam("374320", 100)).unwrap();
        let second = upsert(&conn, &game.id, SourceKind::Steam, &steam("374320", 160)).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.source_playtime_minutes, Some(160));
        assert_eq!(list_for_game(&conn, &game.id).unwrap().len(), 1);
    }

    #[test]
    fn same_external_id_on_other_platform_is_distinct() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let game = games::insert(&conn, "Cyberpunk 2077").unwrap();
        upsert(&conn, &game.id, SourceKind::Steam, &steam("1", 1)).unwrap();
        upsert(&conn, &game.id, SourceKind::Gog, &steam("1", 1)).unwrap();
        assert_eq!(list_for_game(&conn, &game.id).unwrap().len(), 2);
    }

    #[test]
    fn sources_are_deleted_with_their_game() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let game = games::insert(&conn, "Wartales").unwrap();
        upsert(&conn, &game.id, SourceKind::Steam, &steam("1527950", 5)).unwrap();
        games::delete(&conn, &game.id).unwrap();
        assert!(find_by_external(&conn, SourceKind::Steam, "1527950")
            .unwrap()
            .is_none());
    }
}
