//! Collections: manual (hand-picked games) or smart (filter rules).

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::new_id;
use crate::error::{AppError, AppResult};
use crate::library::rules::Rule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CollectionKind {
    Manual,
    Smart,
}

impl CollectionKind {
    fn as_str(self) -> &'static str {
        match self {
            CollectionKind::Manual => "manual",
            CollectionKind::Smart => "smart",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    /// JSON rules (smart collections), ARCHITECTURE.md § 6.
    pub rules: Option<String>,
    pub position: u32,
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<Collection> {
    let kind: String = r.get(2)?;
    Ok(Collection {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: if kind == "smart" {
            CollectionKind::Smart
        } else {
            CollectionKind::Manual
        },
        rules: r.get(3)?,
        position: r.get::<_, i64>(4)?.clamp(0, u32::MAX as i64) as u32,
    })
}

fn clean_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 60 {
        return Err(AppError::Invalid(
            "collection name must be 1-60 characters".into(),
        ));
    }
    Ok(n.to_string())
}

fn check_rules(kind: CollectionKind, rules: Option<&str>) -> AppResult<Option<String>> {
    match (kind, rules) {
        (CollectionKind::Smart, Some(json)) => {
            Rule::parse(json)?;
            Ok(Some(json.to_string()))
        }
        (CollectionKind::Smart, None) => {
            Err(AppError::Invalid("a smart collection needs rules".into()))
        }
        (CollectionKind::Manual, _) => Ok(None),
    }
}

pub fn list(conn: &Connection) -> AppResult<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, rules, position FROM collections ORDER BY position, name",
    )?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Collection> {
    conn.query_row(
        "SELECT id, name, kind, rules, position FROM collections WHERE id = ?1",
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("collection {id}")))
}

pub fn create(
    conn: &Connection,
    name: &str,
    kind: CollectionKind,
    rules: Option<&str>,
) -> AppResult<Collection> {
    let name = clean_name(name)?;
    let rules = check_rules(kind, rules)?;
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM collections",
        [],
        |r| r.get(0),
    )?;
    let id = new_id();
    conn.execute(
        "INSERT INTO collections (id, name, kind, rules, position) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, kind.as_str(), rules, position],
    )?;
    get(conn, &id)
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    rules: Option<&str>,
) -> AppResult<Collection> {
    let current = get(conn, id)?;
    let name = clean_name(name)?;
    let rules = check_rules(current.kind, rules)?;
    conn.execute(
        "UPDATE collections SET name = ?2, rules = ?3 WHERE id = ?1",
        params![id, name, rules],
    )?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_member(
    conn: &Connection,
    collection_id: &str,
    game_id: &str,
    member: bool,
) -> AppResult<()> {
    if get(conn, collection_id)?.kind != CollectionKind::Manual {
        return Err(AppError::Invalid(
            "games are added to manual collections only".into(),
        ));
    }
    if member {
        conn.execute(
            "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
            params![collection_id, game_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
            params![collection_id, game_id],
        )?;
    }
    Ok(())
}

pub fn members(conn: &Connection, collection_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT game_id FROM collection_games WHERE collection_id = ?1")?;
    let rows = stmt.query_map([collection_id], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games;
    use crate::db::Db;

    #[test]
    fn manual_and_smart_collections() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        let manual = create(&conn, "Ce week-end", CollectionKind::Manual, None).unwrap();
        set_member(&conn, &manual.id, &g.id, true).unwrap();
        set_member(&conn, &manual.id, &g.id, true).unwrap();
        assert_eq!(members(&conn, &manual.id).unwrap(), vec![g.id.clone()]);

        let rules = r#"{"all":[{"field":"tag","op":"in","value":["souls-like"]}]}"#;
        let smart = create(&conn, "Souls", CollectionKind::Smart, Some(rules)).unwrap();
        assert_eq!(smart.position, 1);
        assert!(set_member(&conn, &smart.id, &g.id, true).is_err());
        assert!(create(
            &conn,
            "Bad",
            CollectionKind::Smart,
            Some(r#"{"field":"installed","op":"gt","value":1}"#)
        )
        .is_err());
        assert!(create(&conn, "  ", CollectionKind::Manual, None).is_err());

        let renamed = update(&conn, &smart.id, "Souls-like", Some(rules)).unwrap();
        assert_eq!(renamed.name, "Souls-like");
        assert_eq!(list(&conn).unwrap().len(), 2);
        delete(&conn, &manual.id).unwrap();
        assert!(members(&conn, &manual.id).unwrap().is_empty());
    }
}
