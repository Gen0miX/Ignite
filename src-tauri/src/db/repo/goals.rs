//! Personal goals: a checklist per game (replaces or complements achievements).

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::db::{new_id, now_rfc3339};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub position: u32,
    /// RFC 3339 when checked.
    pub done_at: Option<String>,
}

const MAX_TITLE: usize = 200;

fn from_row(r: &Row<'_>) -> rusqlite::Result<Goal> {
    Ok(Goal {
        id: r.get(0)?,
        title: r.get(1)?,
        position: r.get::<_, i64>(2)?.clamp(0, u32::MAX as i64) as u32,
        done_at: r.get(3)?,
    })
}

fn clean_title(title: &str) -> AppResult<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(AppError::Invalid("goal is empty".into()));
    }
    if t.chars().count() > MAX_TITLE {
        return Err(AppError::Invalid("goal is too long".into()));
    }
    Ok(t.to_string())
}

pub fn list(conn: &Connection, game_id: &str) -> AppResult<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, position, done_at FROM goals WHERE game_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([game_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn add(conn: &Connection, game_id: &str, title: &str) -> AppResult<Goal> {
    let title = clean_title(title)?;
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM goals WHERE game_id = ?1",
        [game_id],
        |r| r.get(0),
    )?;
    let goal = Goal {
        id: new_id(),
        title,
        position: position as u32,
        done_at: None,
    };
    conn.execute(
        "INSERT INTO goals (id, game_id, title, position) VALUES (?1, ?2, ?3, ?4)",
        params![goal.id, game_id, goal.title, goal.position],
    )?;
    Ok(goal)
}

fn changed(n: usize, id: &str) -> AppResult<()> {
    if n == 0 {
        return Err(AppError::NotFound(format!("goal {id}")));
    }
    Ok(())
}

pub fn set_done(conn: &Connection, id: &str, done: bool) -> AppResult<()> {
    let done_at = done.then(now_rfc3339);
    changed(
        conn.execute(
            "UPDATE goals SET done_at = ?2 WHERE id = ?1",
            params![id, done_at],
        )?,
        id,
    )
}

pub fn rename(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    let title = clean_title(title)?;
    changed(
        conn.execute(
            "UPDATE goals SET title = ?2 WHERE id = ?1",
            params![id, title],
        )?,
        id,
    )
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    changed(conn.execute("DELETE FROM goals WHERE id = ?1", [id])?, id)
}

/// `(done, total)` per game, for summaries.
pub fn progress(conn: &Connection) -> AppResult<Vec<(String, u32, u32)>> {
    let mut stmt = conn.prepare(
        "SELECT game_id, SUM(done_at IS NOT NULL), COUNT(*) FROM goals GROUP BY game_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)? as u32,
            r.get::<_, i64>(2)? as u32,
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    #[test]
    fn checklist_lifecycle() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Minecraft").unwrap();
        let a = add(&conn, &g.id, " Tuer l'Ender Dragon ").unwrap();
        let b = add(&conn, &g.id, "Construire un phare").unwrap();
        assert_eq!((a.position, b.position), (0, 1));
        assert_eq!(a.title, "Tuer l'Ender Dragon");

        set_done(&conn, &a.id, true).unwrap();
        rename(&conn, &b.id, "Construire un grand phare").unwrap();
        let list = list(&conn, &g.id).unwrap();
        assert!(list[0].done_at.is_some());
        assert_eq!(list[1].title, "Construire un grand phare");
        assert_eq!(progress(&conn).unwrap(), vec![(g.id.clone(), 1, 2)]);

        set_done(&conn, &a.id, false).unwrap();
        delete(&conn, &b.id).unwrap();
        assert_eq!(progress(&conn).unwrap(), vec![(g.id.clone(), 0, 1)]);
        assert!(add(&conn, &g.id, "  ").is_err());
        assert!(matches!(delete(&conn, "nope"), Err(AppError::NotFound(_))));
    }
}
