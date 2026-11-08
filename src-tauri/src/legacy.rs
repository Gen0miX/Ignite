//! One-time move of the data stored under the former `com.ludotheque.app`
//! identifier (`%APPDATA%` folder with `ludotheque.db`, `%LOCALAPPDATA%`
//! folder with the WebView data) to `com.ignite.app` / `ignite.db`.
//!
//! Runs before the database and the window open. A move that fails (the old
//! version still running, a file locked) never starts on an empty library:
//! the old folder and file keep being used and the move is retried next time.

use std::fs;
use std::path::{Path, PathBuf};

use crate::db::DB_FILE_NAME;

/// Must match `identifier` in `tauri.conf.json` (checked by a test).
pub const APP_ID: &str = "com.ignite.app";
pub const LEGACY_ID: &str = "com.ludotheque.app";
pub const LEGACY_DB_FILE_NAME: &str = "ludotheque.db";

/// SQLite files that travel with the database.
const DB_SUFFIXES: [&str; 3] = ["", "-wal", "-shm"];

/// Moves `<base>/LEGACY_ID` to `<base>/APP_ID` when the new folder holds no
/// data yet. Returns the folder to use: the old one if the move failed.
pub fn move_dir(base: &Path) -> PathBuf {
    let old = base.join(LEGACY_ID);
    let new = base.join(APP_ID);
    if !old.is_dir() {
        return new;
    }
    if new.exists() {
        // Already migrated (or used by a newer build): the leftover old
        // folder stays untouched. An empty new folder is replaced.
        if fs::remove_dir(&new).is_err() {
            return new;
        }
    }
    if let Err(e) = fs::rename(&old, &new) {
        // A handle on the folder itself (an Explorer or shell window opened
        // there, an antivirus scan) refuses the rename while its contents
        // can still move: move them one by one.
        tracing::info!(error = %e, path = %old.display(), "data folder busy, moving its contents");
        if let Err(e) = move_contents(&old, &new) {
            tracing::warn!(error = %e, path = %old.display(), "data folder not moved, kept in place");
            return old;
        }
        // The emptied folder may still be held: it goes on a later start.
        let _ = fs::remove_dir(&old);
    }
    tracing::info!(from = %old.display(), to = %new.display(), "data folder moved");
    new
}

/// Moves every entry of `old` into a new `new` folder; on a failure, puts
/// back what was moved and removes `new`, so the data is never split.
fn move_contents(old: &Path, new: &Path) -> std::io::Result<()> {
    fs::create_dir(new)?;
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result = (|| {
        for entry in fs::read_dir(old)? {
            let from = entry?.path();
            let to = new.join(from.file_name().unwrap_or_default());
            fs::rename(&from, &to)?;
            moved.push((from, to));
        }
        Ok(())
    })();
    if result.is_err() {
        for (from, to) in moved.iter().rev() {
            let _ = fs::rename(to, from);
        }
        let _ = fs::remove_dir(new);
    }
    result
}

/// Database path in `dir`: renames `ludotheque.db` (with its `-wal`/`-shm`)
/// to `ignite.db` when needed, or keeps the old name if that fails.
pub fn db_path(dir: &Path) -> PathBuf {
    let new = dir.join(DB_FILE_NAME);
    let old = dir.join(LEGACY_DB_FILE_NAME);
    if new.exists() || !old.exists() {
        return new;
    }
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    for suffix in DB_SUFFIXES {
        let from = dir.join(format!("{LEGACY_DB_FILE_NAME}{suffix}"));
        let to = dir.join(format!("{DB_FILE_NAME}{suffix}"));
        if !from.exists() {
            continue;
        }
        if let Err(e) = fs::rename(&from, &to) {
            tracing::warn!(error = %e, file = %from.display(), "database not renamed, kept in place");
            // The journal must stay next to its database: undo.
            for (from, to) in done.iter().rev() {
                let _ = fs::rename(to, from);
            }
            return old;
        }
        done.push((from, to));
    }
    tracing::info!(path = %new.display(), "database renamed");
    new
}

/// Moves both folders; returns the data folder to use instead of Tauri's
/// `app_data_dir` when the old one had to be kept.
#[cfg(windows)]
pub fn migrate() -> Option<PathBuf> {
    // WebView data (UI preferences): losing it only resets them.
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        move_dir(Path::new(&local));
    }
    let roaming = std::env::var_os("APPDATA")?;
    let roaming = Path::new(&roaming);
    let dir = move_dir(roaming);
    (dir != roaming.join(APP_ID)).then_some(dir)
}

#[cfg(not(windows))]
pub fn migrate() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn app_id_matches_the_tauri_config() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], APP_ID);
    }

    #[test]
    fn moves_the_old_folder_and_renames_the_database() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join(LEGACY_ID);
        write(&old.join("ludotheque.db"), "db");
        write(&old.join("ludotheque.db-wal"), "wal");
        write(&old.join("artwork/g1/cover.jpg"), "img");

        let dir = move_dir(base.path());
        assert_eq!(dir, base.path().join(APP_ID));
        assert!(!old.exists());
        let db = db_path(&dir);
        assert_eq!(db, dir.join("ignite.db"));
        assert_eq!(fs::read_to_string(&db).unwrap(), "db");
        assert_eq!(
            fs::read_to_string(dir.join("ignite.db-wal")).unwrap(),
            "wal"
        );
        assert!(!dir.join("ludotheque.db").exists());
        assert!(dir.join("artwork/g1/cover.jpg").exists());
    }

    #[test]
    fn replaces_an_empty_new_folder() {
        let base = tempfile::tempdir().unwrap();
        write(&base.path().join(LEGACY_ID).join("ludotheque.db"), "db");
        fs::create_dir_all(base.path().join(APP_ID)).unwrap();
        let dir = move_dir(base.path());
        assert_eq!(dir, base.path().join(APP_ID));
        assert!(dir.join("ludotheque.db").exists());
    }

    #[test]
    fn keeps_data_already_migrated() {
        let base = tempfile::tempdir().unwrap();
        write(&base.path().join(LEGACY_ID).join("ludotheque.db"), "old");
        write(&base.path().join(APP_ID).join("ignite.db"), "new");
        let dir = move_dir(base.path());
        assert_eq!(dir, base.path().join(APP_ID));
        assert_eq!(fs::read_to_string(db_path(&dir)).unwrap(), "new");
        // The leftover is not deleted.
        assert!(base.path().join(LEGACY_ID).join("ludotheque.db").exists());
    }

    #[test]
    fn fresh_install_uses_the_new_names() {
        let base = tempfile::tempdir().unwrap();
        let dir = move_dir(base.path());
        assert_eq!(dir, base.path().join(APP_ID));
        assert_eq!(db_path(&dir), dir.join("ignite.db"));
    }

    /// Opens like a process that does not let others delete or rename it.
    #[cfg(windows)]
    fn hold(path: &Path) -> fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000; // needed for a folder
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn a_held_folder_has_its_contents_moved() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join(LEGACY_ID);
        write(&old.join("ludotheque.db"), "db");
        write(&old.join("artwork/g1/cover.jpg"), "img");
        let held = hold(&old);
        assert!(fs::rename(&old, base.path().join("probe")).is_err());

        let dir = move_dir(base.path());
        assert_eq!(dir, base.path().join(APP_ID));
        assert_eq!(fs::read_to_string(dir.join("ludotheque.db")).unwrap(), "db");
        assert!(dir.join("artwork/g1/cover.jpg").exists());
        assert_eq!(fs::read_dir(&old).unwrap().count(), 0);
        drop(held);
        // Next start: the empty leftover does not hide the moved data.
        assert_eq!(move_dir(base.path()), base.path().join(APP_ID));
    }

    #[cfg(windows)]
    #[test]
    fn a_held_file_keeps_everything_in_the_old_folder() {
        let base = tempfile::tempdir().unwrap();
        let old = base.path().join(LEGACY_ID);
        write(&old.join("a.txt"), "a");
        write(&old.join("ludotheque.db"), "db");
        write(&old.join("z.txt"), "z");
        let _held = hold(&old.join("ludotheque.db"));

        assert_eq!(move_dir(base.path()), old);
        for name in ["a.txt", "ludotheque.db", "z.txt"] {
            assert!(old.join(name).exists(), "{name} put back");
        }
        assert!(!base.path().join(APP_ID).exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_locked_database_keeps_its_old_name() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("ludotheque.db"), "db");
        write(&dir.path().join("ludotheque.db-wal"), "wal");
        // Opened without FILE_SHARE_DELETE, like a running old version.
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(dir.path().join("ludotheque.db-wal"))
            .unwrap();
        assert_eq!(db_path(dir.path()), dir.path().join("ludotheque.db"));
        assert!(dir.path().join("ludotheque.db").exists());
        assert!(!dir.path().join("ignite.db").exists());
    }
}
