//! Play sessions. Only aggregates for now; the tracker (J2) adds writes.

use rusqlite::Connection;

use crate::error::AppResult;

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
