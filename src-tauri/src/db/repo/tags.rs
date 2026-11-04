use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::new_id;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Tag {
    pub id: String,
    pub name: String,
}

const MAX_TAG_LEN: usize = 40;

pub fn normalize_name(name: &str) -> AppResult<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(AppError::Invalid("tag is empty".into()));
    }
    if name.chars().count() > MAX_TAG_LEN {
        return Err(AppError::Invalid("tag is too long".into()));
    }
    Ok(name)
}

pub fn for_game(conn: &Connection, game_id: &str) -> AppResult<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name FROM tags t JOIN game_tags gt ON gt.tag_id = t.id \
         WHERE gt.game_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([game_id], |r| {
        Ok(Tag {
            id: r.get(0)?,
            name: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Adds a user tag to a game, reusing an existing tag with the same name
/// (case-insensitive).
pub fn add_to_game(conn: &Connection, game_id: &str, name: &str) -> AppResult<Tag> {
    let name = normalize_name(name)?;
    let existing: Option<Tag> = conn
        .query_row(
            "SELECT id, name FROM tags WHERE name = ?1 COLLATE NOCASE",
            [&name],
            |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            },
        )
        .optional()?;
    let tag = match existing {
        Some(t) => t,
        None => {
            let tag = Tag { id: new_id(), name };
            conn.execute(
                "INSERT INTO tags (id, name, origin) VALUES (?1, ?2, 'user')",
                params![tag.id, tag.name],
            )?;
            tag
        }
    };
    conn.execute(
        "INSERT OR IGNORE INTO game_tags (game_id, tag_id) VALUES (?1, ?2)",
        params![game_id, tag.id],
    )?;
    Ok(tag)
}

pub fn remove_from_game(conn: &Connection, game_id: &str, tag_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM game_tags WHERE game_id = ?1 AND tag_id = ?2",
        params![game_id, tag_id],
    )?;
    // Drop user tags nobody uses anymore.
    conn.execute(
        "DELETE FROM tags WHERE id = ?1 AND origin = 'user' \
         AND NOT EXISTS (SELECT 1 FROM game_tags WHERE tag_id = ?1)",
        [tag_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    #[test]
    fn add_reuses_tags_case_insensitively_and_remove_cleans_up() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let a = games::insert(&conn, "A").unwrap();
        let b = games::insert(&conn, "B").unwrap();
        let t1 = add_to_game(&conn, &a.id, "  Souls-like ").unwrap();
        let t2 = add_to_game(&conn, &b.id, "souls-LIKE").unwrap();
        assert_eq!(t1.id, t2.id);
        assert_eq!(t1.name, "Souls-like");
        add_to_game(&conn, &a.id, "Souls-like").unwrap(); // idempotent
        assert_eq!(for_game(&conn, &a.id).unwrap().len(), 1);

        remove_from_game(&conn, &a.id, &t1.id).unwrap();
        assert!(for_game(&conn, &a.id).unwrap().is_empty());
        assert_eq!(for_game(&conn, &b.id).unwrap(), vec![t2.clone()]);
        remove_from_game(&conn, &b.id, &t2.id).unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn rejects_bad_names() {
        assert!(normalize_name("   ").is_err());
        assert!(normalize_name(&"x".repeat(41)).is_err());
        assert_eq!(normalize_name("a   b").unwrap(), "a b");
    }
}
