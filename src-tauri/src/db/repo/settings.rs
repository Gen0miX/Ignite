//! Key/value app settings stored as JSON. Never store secrets here (see `secrets`).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppResult;

pub fn get_raw(conn: &Connection, key: &str) -> AppResult<Option<serde_json::Value>> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw.map(|s| serde_json::from_str(&s)).transpose()?)
}

pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> AppResult<Option<T>> {
    Ok(get_raw(conn, key)?
        .map(serde_json::from_value)
        .transpose()?)
}

pub fn set<T: Serialize + ?Sized>(conn: &Connection, key: &str, value: &T) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, serde_json::to_string(value)?],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, key: &str) -> AppResult<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn set_get_overwrite_delete() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let key = "tracking.min_session_minutes";
        assert_eq!(get::<u32>(&conn, key).unwrap(), None);
        set(&conn, key, &2u32).unwrap();
        set(&conn, key, &5u32).unwrap();
        assert_eq!(get::<u32>(&conn, key).unwrap(), Some(5));
        delete(&conn, key).unwrap();
        assert_eq!(get_raw(&conn, key).unwrap(), None);
    }

    #[test]
    fn stores_strings_with_backslashes() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        set(&conn, "steam.path", "C:\\Program Files (x86)\\Steam").unwrap();
        let v: String = get(&conn, "steam.path").unwrap().unwrap();
        assert_eq!(v, "C:\\Program Files (x86)\\Steam");
    }
}
