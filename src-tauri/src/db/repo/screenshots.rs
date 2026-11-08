//! Captures of a game (docs/FICHE-ONGLETS.md): metadata only, the files stay
//! where Steam, the Game Bar or the user put them. Rows are keyed by path so a
//! rescan keeps the caption the user wrote.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSource {
    Steam,
    Gamebar,
    Folder,
}

impl CaptureSource {
    pub fn as_str(self) -> &'static str {
        match self {
            CaptureSource::Steam => "steam",
            CaptureSource::Gamebar => "gamebar",
            CaptureSource::Folder => "folder",
        }
    }

    fn parse(s: &str) -> rusqlite::Result<Self> {
        match s {
            "steam" => Ok(CaptureSource::Steam),
            "gamebar" => Ok(CaptureSource::Gamebar),
            "folder" => Ok(CaptureSource::Folder),
            other => Err(bad_text(2, "source", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CaptureKind {
    Image,
    Video,
}

impl CaptureKind {
    fn as_str(self) -> &'static str {
        match self {
            CaptureKind::Image => "image",
            CaptureKind::Video => "video",
        }
    }

    fn parse(s: &str) -> rusqlite::Result<Self> {
        match s {
            "image" => Ok(CaptureKind::Image),
            "video" => Ok(CaptureKind::Video),
            other => Err(bad_text(4, "kind", other)),
        }
    }
}

fn bad_text(col: usize, what: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnType(col, format!("{what} {value}"), rusqlite::types::Type::Text)
}

/// A capture found on disk by a scan.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundCapture {
    pub game_id: String,
    pub source: CaptureSource,
    pub path: String,
    pub kind: CaptureKind,
    /// UTC, `YYYY-MM-DDTHH:MM:SSZ` (sorts as text).
    pub taken_at: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: u64,
    /// A thumbnail the source already provides (Steam).
    pub thumb_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    pub id: String,
    pub game_id: String,
    pub source: CaptureSource,
    pub path: String,
    pub kind: CaptureKind,
    pub taken_at: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// File size in bytes (capped at 4 GB).
    pub bytes: u32,
    pub caption: Option<String>,
    pub thumb_path: Option<String>,
}

const COLUMNS: &str =
    "id, game_id, source, path, kind, taken_at, width, height, bytes, caption, thumb_path";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Capture> {
    let dim = |v: Option<i64>| v.map(|v| v.clamp(0, u32::MAX as i64) as u32);
    Ok(Capture {
        id: r.get(0)?,
        game_id: r.get(1)?,
        source: CaptureSource::parse(&r.get::<_, String>(2)?)?,
        path: r.get(3)?,
        kind: CaptureKind::parse(&r.get::<_, String>(4)?)?,
        taken_at: r.get(5)?,
        width: dim(r.get(6)?),
        height: dim(r.get(7)?),
        bytes: r.get::<_, i64>(8)?.clamp(0, u32::MAX as i64) as u32,
        caption: r.get(9)?,
        thumb_path: r.get(10)?,
    })
}

/// Inserts or refreshes a capture; the caption (and a generated thumbnail,
/// when the source gives none) survive the rescan.
pub fn upsert(conn: &Connection, c: &FoundCapture) -> AppResult<()> {
    conn.execute(
        "INSERT INTO screenshots (id, game_id, source, path, kind, taken_at, width, height, \
           bytes, thumb_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         ON CONFLICT(path) DO UPDATE SET game_id = excluded.game_id, source = excluded.source, \
           kind = excluded.kind, taken_at = excluded.taken_at, \
           width = COALESCE(excluded.width, screenshots.width), \
           height = COALESCE(excluded.height, screenshots.height), \
           bytes = excluded.bytes, \
           thumb_path = COALESCE(excluded.thumb_path, screenshots.thumb_path)",
        params![
            crate::db::new_id(),
            c.game_id,
            c.source.as_str(),
            c.path,
            c.kind.as_str(),
            c.taken_at,
            c.width,
            c.height,
            c.bytes.min(i64::MAX as u64) as i64,
            c.thumb_path,
        ],
    )?;
    Ok(())
}

/// Every known path (to skip unchanged files during a scan).
pub fn known_paths(conn: &Connection) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT path FROM screenshots")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Forgets the captures `drop` accepts (file gone, folder removed). Returns the generated thumbnails of the
/// removed rows, for the caller to clean up.
pub fn remove_where(
    conn: &Connection,
    mut drop: impl FnMut(&Capture) -> bool,
) -> AppResult<Vec<String>> {
    let all = list_all(conn)?;
    let mut thumbs = Vec::new();
    for c in all.iter().filter(|c| drop(c)) {
        conn.execute("DELETE FROM screenshots WHERE id = ?1", [&c.id])?;
        thumbs.extend(c.thumb_path.clone());
    }
    Ok(thumbs)
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<Capture>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM screenshots"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Captures of a game, newest first.
pub fn list_for_game(conn: &Connection, game_id: &str) -> AppResult<Vec<Capture>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM screenshots WHERE game_id = ?1 ORDER BY taken_at DESC, path"
    ))?;
    let rows = stmt.query_map([game_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Capture> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM screenshots WHERE id = ?1"),
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("capture {id}")))
}

pub fn set_caption(conn: &Connection, id: &str, caption: Option<&str>) -> AppResult<()> {
    let caption = caption.map(str::trim).filter(|c| !c.is_empty());
    let n = conn.execute(
        "UPDATE screenshots SET caption = ?2 WHERE id = ?1",
        params![id, caption],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("capture {id}")));
    }
    Ok(())
}

pub fn set_thumb(conn: &Connection, id: &str, thumb: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE screenshots SET thumb_path = ?2 WHERE id = ?1",
        params![id, thumb],
    )?;
    Ok(())
}

/// Images still without a thumbnail: `(id, path)`.
pub fn missing_thumbs(conn: &Connection) -> AppResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, path FROM screenshots WHERE kind = 'image' AND thumb_path IS NULL \
         ORDER BY taken_at DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// --- Folders added by the user ----------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureFolder {
    pub path: String,
    /// `None` = "all games": files are matched to games by name.
    pub game_id: Option<String>,
    pub game_title: Option<String>,
}

pub fn folders(conn: &Connection) -> AppResult<Vec<CaptureFolder>> {
    let mut stmt = conn.prepare(
        "SELECT f.path, f.game_id, g.title FROM screenshot_folders f \
         LEFT JOIN games g ON g.id = f.game_id ORDER BY f.path",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(CaptureFolder {
            path: r.get(0)?,
            game_id: r.get(1)?,
            game_title: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn add_folder(conn: &Connection, path: &str, game_id: Option<&str>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO screenshot_folders (path, game_id) VALUES (?1, ?2) \
         ON CONFLICT(path) DO UPDATE SET game_id = excluded.game_id",
        params![path, game_id],
    )?;
    Ok(())
}

pub fn remove_folder(conn: &Connection, path: &str) -> AppResult<()> {
    conn.execute("DELETE FROM screenshot_folders WHERE path = ?1", [path])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games;
    use crate::db::Db;

    fn found(game: &str, path: &str, at: &str) -> FoundCapture {
        FoundCapture {
            game_id: game.into(),
            source: CaptureSource::Steam,
            path: path.into(),
            kind: CaptureKind::Image,
            taken_at: at.into(),
            width: Some(2560),
            height: Some(1440),
            bytes: 431_618,
            thumb_path: None,
        }
    }

    #[test]
    fn rescans_keep_the_caption_and_the_thumbnail() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Endless Space 2").unwrap();
        upsert(&conn, &found(&g.id, r"C:\s\1.jpg", "2025-10-29T10:22:31Z")).unwrap();
        let id = list_for_game(&conn, &g.id).unwrap()[0].id.clone();
        set_caption(&conn, &id, Some("  Première alliance  ")).unwrap();
        set_thumb(&conn, &id, r"C:\thumbs\a.jpg").unwrap();

        upsert(&conn, &found(&g.id, r"C:\s\1.jpg", "2025-10-29T10:22:31Z")).unwrap();
        let list = list_for_game(&conn, &g.id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].caption.as_deref(), Some("Première alliance"));
        assert_eq!(list[0].thumb_path.as_deref(), Some(r"C:\thumbs\a.jpg"));
        assert!(missing_thumbs(&conn).unwrap().is_empty());

        set_caption(&conn, &id, Some(" ")).unwrap();
        assert_eq!(get(&conn, &id).unwrap().caption, None);
    }

    #[test]
    fn lists_newest_first_and_forgets_removed_files() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        upsert(&conn, &found(&g.id, "a.jpg", "2025-01-01T10:00:00Z")).unwrap();
        upsert(&conn, &found(&g.id, "b.jpg", "2025-03-01T10:00:00Z")).unwrap();
        let paths: Vec<_> = list_for_game(&conn, &g.id)
            .unwrap()
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(paths, ["b.jpg", "a.jpg"]);
        remove_where(&conn, |c| c.path == "a.jpg").unwrap();
        assert_eq!(list_all(&conn).unwrap().len(), 1);
    }

    #[test]
    fn folders_are_tied_to_a_game_or_to_all() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        add_folder(&conn, r"D:\Captures", None).unwrap();
        add_folder(&conn, r"D:\Hades", Some(&g.id)).unwrap();
        let list = folders(&conn).unwrap();
        assert_eq!(list[1].game_title.as_deref(), Some("Hades"));
        assert_eq!(list[0].game_id, None);
        remove_folder(&conn, r"D:\Captures").unwrap();
        assert_eq!(folders(&conn).unwrap().len(), 1);
    }
}
