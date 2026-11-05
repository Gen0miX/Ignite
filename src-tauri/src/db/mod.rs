//! SQLite access: connection setup, migrations and repositories.

pub mod repo;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use crate::error::AppResult;

pub const DB_FILE_NAME: &str = "ludotheque.db";

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_initial.sql")),
        M::up(include_str!("../../migrations/0002_artwork_header.sql")),
        M::up(include_str!("../../migrations/0003_achievements_cache.sql")),
        M::up(include_str!("../../migrations/0004_stats.sql")),
        M::up(include_str!("../../migrations/0005_store_meta.sql")),
    ])
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Thread-safe handle to the app database, stored in Tauri state.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Opens (or creates) the database file and applies pending migrations.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        configure(&conn)?;
        migrations().to_latest(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// In-memory database with the full schema, for tests.
    pub fn open_in_memory() -> AppResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        configure(&conn)?;
        migrations().to_latest(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A poisoned lock only means another thread panicked mid-query; the
        // connection itself is still usable.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Current UTC time as an RFC 3339 string (the format used in every TEXT date column).
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().unwrap();
    }

    #[test]
    fn migration_creates_all_tables() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            tables,
            [
                "achievements",
                "artwork",
                "collection_games",
                "collections",
                "game_sources",
                "game_tags",
                "games",
                "goals",
                "launch_profiles",
                "play_sessions",
                "playtime_snapshots",
                "settings",
                "tags",
            ]
        );
    }

    #[test]
    fn open_creates_file_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("ludotheque-test-{}", new_id()));
        let path = dir.join(DB_FILE_NAME);
        Db::open(&path).unwrap();
        assert!(path.exists());
        let db = Db::open(&path).unwrap(); // re-open: no migration error
        let version: i64 = db
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 5);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Db::open_in_memory().unwrap();
        let err = db.conn().execute(
            "INSERT INTO game_sources (id, game_id, kind) VALUES ('s1', 'missing', 'steam')",
            [],
        );
        assert!(err.is_err());
    }
}
