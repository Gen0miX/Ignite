//! Play sessions: written by the tracker (`auto`) and by the user
//! (`manual`, `estimate`); `imported` comes from launchers (J3).

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SessionOrigin {
    Auto,
    Manual,
    Imported,
    Estimate,
}

impl SessionOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionOrigin::Auto => "auto",
            SessionOrigin::Manual => "manual",
            SessionOrigin::Imported => "imported",
            SessionOrigin::Estimate => "estimate",
        }
    }

    fn parse(s: &str) -> rusqlite::Result<Self> {
        match s {
            "auto" => Ok(SessionOrigin::Auto),
            "manual" => Ok(SessionOrigin::Manual),
            "imported" => Ok(SessionOrigin::Imported),
            "estimate" => Ok(SessionOrigin::Estimate),
            other => Err(rusqlite::Error::InvalidColumnType(
                0,
                format!("origin '{other}'"),
                rusqlite::types::Type::Text,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaySession {
    pub id: String,
    pub game_id: String,
    pub game_source_id: Option<String>,
    /// RFC 3339; `None` for an "already played" estimate.
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// Active time (idle removed).
    pub duration_seconds: u32,
    pub idle_seconds: u32,
    pub origin: SessionOrigin,
    pub note: Option<String>,
}

const COLUMNS: &str = "id, game_id, game_source_id, started_at, ended_at, duration_seconds, \
     idle_seconds, origin, note";

fn from_row(r: &Row<'_>) -> rusqlite::Result<PlaySession> {
    let to_u32 = |v: i64| v.clamp(0, u32::MAX as i64) as u32;
    Ok(PlaySession {
        id: r.get(0)?,
        game_id: r.get(1)?,
        game_source_id: r.get(2)?,
        started_at: r.get(3)?,
        ended_at: r.get(4)?,
        duration_seconds: to_u32(r.get(5)?),
        idle_seconds: to_u32(r.get(6)?),
        origin: SessionOrigin::parse(&r.get::<_, String>(7)?)?,
        note: r.get(8)?,
    })
}

/// Inserts or replaces a whole session row.
pub fn save(conn: &Connection, s: &PlaySession) -> AppResult<()> {
    conn.execute(
        "INSERT INTO play_sessions (id, game_id, game_source_id, started_at, ended_at, \
           duration_seconds, idle_seconds, origin, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(id) DO UPDATE SET game_source_id = excluded.game_source_id, \
           started_at = excluded.started_at, ended_at = excluded.ended_at, \
           duration_seconds = excluded.duration_seconds, idle_seconds = excluded.idle_seconds, \
           origin = excluded.origin, note = excluded.note",
        params![
            s.id,
            s.game_id,
            s.game_source_id,
            s.started_at,
            s.ended_at,
            s.duration_seconds,
            s.idle_seconds,
            s.origin.as_str(),
            s.note
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<PlaySession> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM play_sessions WHERE id = ?1"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("session {id}")))
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM play_sessions WHERE id = ?1", [id])?;
    Ok(())
}

/// An idle stretch removed from a session (RFC 3339).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IdlePeriod {
    pub started_at: String,
    pub ended_at: String,
}

/// Replaces the idle stretches of a session.
pub fn replace_idle_periods(
    conn: &Connection,
    session_id: &str,
    periods: &[IdlePeriod],
) -> AppResult<()> {
    conn.execute(
        "DELETE FROM session_idle_periods WHERE session_id = ?1",
        [session_id],
    )?;
    let mut stmt = conn.prepare(
        "INSERT INTO session_idle_periods (session_id, started_at, ended_at) VALUES (?1, ?2, ?3)",
    )?;
    for p in periods {
        stmt.execute(params![session_id, p.started_at, p.ended_at])?;
    }
    Ok(())
}

pub fn idle_periods(conn: &Connection, session_id: &str) -> AppResult<Vec<IdlePeriod>> {
    let mut stmt = conn.prepare(
        "SELECT started_at, ended_at FROM session_idle_periods WHERE session_id = ?1 \
         ORDER BY started_at",
    )?;
    let rows = stmt.query_map([session_id], |r| {
        Ok(IdlePeriod {
            started_at: r.get(0)?,
            ended_at: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Rating of a session, 1 to 5 flames (`None` clears it).
pub fn set_rating(conn: &Connection, id: &str, rating: Option<u8>) -> AppResult<()> {
    if rating.is_some_and(|r| !(1..=5).contains(&r)) {
        return Err(AppError::Invalid("rating must be between 1 and 5".into()));
    }
    conn.execute(
        "UPDATE play_sessions SET rating = ?2 WHERE id = ?1",
        params![id, rating],
    )?;
    Ok(())
}

pub fn rating(conn: &Connection, id: &str) -> AppResult<Option<u8>> {
    Ok(conn
        .query_row(
            "SELECT rating FROM play_sessions WHERE id = ?1",
            [id],
            |r| r.get::<_, Option<u8>>(0),
        )
        .optional()?
        .flatten())
}

pub fn set_note(conn: &Connection, id: &str, note: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE play_sessions SET note = ?2 WHERE id = ?1",
        params![id, note],
    )?;
    Ok(())
}

/// Deletes automatic sessions shorter than `min_secs`; returns how many.
pub fn purge_short_auto(conn: &Connection, min_secs: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "DELETE FROM play_sessions WHERE origin = 'auto' AND duration_seconds < ?1",
        [min_secs],
    )?)
}

/// Sessions of a game, most recent first (estimates without a date last).
pub fn list_for_game(conn: &Connection, game_id: &str) -> AppResult<Vec<PlaySession>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM play_sessions WHERE game_id = ?1 \
         ORDER BY started_at IS NULL, started_at DESC, id"
    ))?;
    let rows = stmt.query_map([game_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Total of the sessions of one game for one source (`None` = no source).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSum {
    pub game_id: String,
    pub game_source_id: Option<String>,
    pub seconds: i64,
    pub count: i64,
    pub last_ended_at: Option<String>,
}

pub fn sums(conn: &Connection) -> AppResult<Vec<SessionSum>> {
    let mut stmt = conn.prepare(
        "SELECT game_id, game_source_id, SUM(duration_seconds), COUNT(*), MAX(ended_at) \
         FROM play_sessions GROUP BY game_id, game_source_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SessionSum {
            game_id: r.get(0)?,
            game_source_id: r.get(1)?,
            seconds: r.get(2)?,
            count: r.get(3)?,
            last_ended_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    fn session(
        id: &str,
        game: &str,
        start: Option<&str>,
        secs: u32,
        origin: SessionOrigin,
    ) -> PlaySession {
        PlaySession {
            id: id.into(),
            game_id: game.into(),
            game_source_id: None,
            started_at: start.map(Into::into),
            ended_at: None,
            duration_seconds: secs,
            idle_seconds: 0,
            origin,
            note: None,
        }
    }

    #[test]
    fn save_update_list_delete() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        save(
            &conn,
            &session(
                "a",
                &g.id,
                Some("2026-01-01T10:00:00Z"),
                600,
                SessionOrigin::Auto,
            ),
        )
        .unwrap();
        save(
            &conn,
            &session(
                "b",
                &g.id,
                Some("2026-02-01T10:00:00Z"),
                300,
                SessionOrigin::Manual,
            ),
        )
        .unwrap();
        save(
            &conn,
            &session("c", &g.id, None, 36_000, SessionOrigin::Estimate),
        )
        .unwrap();

        let mut a = get(&conn, "a").unwrap();
        a.duration_seconds = 900;
        a.ended_at = Some("2026-01-01T10:15:00Z".into());
        save(&conn, &a).unwrap();

        let ids: Vec<_> = list_for_game(&conn, &g.id)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, ["b", "a", "c"]);
        assert_eq!(get(&conn, "a").unwrap().duration_seconds, 900);
        assert_eq!(sums(&conn).unwrap()[0].seconds, 900 + 300 + 36_000);

        delete(&conn, "b").unwrap();
        assert!(matches!(get(&conn, "b"), Err(AppError::NotFound(_))));
    }
}
