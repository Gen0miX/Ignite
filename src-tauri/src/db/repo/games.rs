use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::{new_id, now_rfc3339};
use crate::error::{AppError, AppResult};
use crate::models::GameStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub sort_title: String,
    pub status: GameStatus,
    pub favorite: bool,
    pub hidden: bool,
    pub notes: Option<String>,
    pub preferred_source_id: Option<String>,
    pub igdb_id: Option<i64>,
    pub steamgriddb_id: Option<i64>,
    pub hltb_main_minutes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields the user can change on a game. `None` leaves the value untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GamePatch {
    pub title: Option<String>,
    pub status: Option<GameStatus>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    /// `Some(None)` clears the note.
    pub notes: Option<Option<String>>,
    pub preferred_source_id: Option<Option<String>>,
}

const COLUMNS: &str =
    "id, title, sort_title, status, favorite, hidden, notes, preferred_source_id, \
     igdb_id, steamgriddb_id, hltb_main_minutes, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Game> {
    Ok(Game {
        id: r.get(0)?,
        title: r.get(1)?,
        sort_title: r.get(2)?,
        status: r.get(3)?,
        favorite: r.get(4)?,
        hidden: r.get(5)?,
        notes: r.get(6)?,
        preferred_source_id: r.get(7)?,
        igdb_id: r.get(8)?,
        steamgriddb_id: r.get(9)?,
        hltb_main_minutes: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

/// "Où t'es-tu arrêté ?": latest resume note of a game and when it was written.
pub fn resume_note(conn: &Connection, id: &str) -> AppResult<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "SELECT resume_note, resume_note_at FROM games WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?
        .and_then(|(n, at)| n.zip(at)))
}

/// Sets (or clears, with `None` / blank) the resume note.
pub fn set_resume_note(conn: &Connection, id: &str, note: Option<&str>) -> AppResult<()> {
    let note = note.map(str::trim).filter(|n| !n.is_empty());
    conn.execute(
        "UPDATE games SET resume_note = ?2, resume_note_at = CASE WHEN ?2 IS NULL THEN NULL ELSE ?3 END \
         WHERE id = ?1",
        params![id, note, now_rfc3339()],
    )?;
    Ok(())
}

const LEADING_ARTICLES: &[&str] = &["the ", "a ", "an ", "les ", "le ", "la ", "l'", "l\u{2019}"];

/// Key used to sort titles: lowercase, accents folded, leading article removed
/// ("The Witcher 3" sorts under W, "Les Sims" under S).
pub fn sort_title(title: &str) -> String {
    let folded: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ñ' => 'n',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' | 'ý' => 'y',
            other => other,
        })
        .collect();
    for article in LEADING_ARTICLES {
        if let Some(rest) = folded.strip_prefix(article) {
            if !rest.trim().is_empty() {
                return rest.trim_start().to_string();
            }
        }
    }
    folded
}

pub fn insert(conn: &Connection, title: &str) -> AppResult<Game> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Invalid("game title is empty".into()));
    }
    let id = new_id();
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO games (id, title, sort_title, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, title, sort_title(title), now],
    )?;
    get(conn, &id)
}

pub fn find(conn: &Connection, id: &str) -> AppResult<Option<Game>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM games WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Game> {
    find(conn, id)?.ok_or_else(|| AppError::NotFound(format!("game {id}")))
}

/// All games, hidden ones excluded unless asked, ordered by sort title.
pub fn list(conn: &Connection, include_hidden: bool) -> AppResult<Vec<Game>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM games WHERE ?1 OR hidden = 0 ORDER BY sort_title, id"
    ))?;
    let rows = stmt.query_map([include_hidden], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn update(conn: &Connection, id: &str, patch: &GamePatch) -> AppResult<Game> {
    let mut game = get(conn, id)?;
    if let Some(title) = &patch.title {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::Invalid("game title is empty".into()));
        }
        game.title = title.to_string();
        game.sort_title = sort_title(title);
    }
    if let Some(status) = patch.status {
        game.status = status;
    }
    if let Some(favorite) = patch.favorite {
        game.favorite = favorite;
    }
    if let Some(hidden) = patch.hidden {
        game.hidden = hidden;
    }
    if let Some(notes) = &patch.notes {
        game.notes = notes.clone();
    }
    if let Some(preferred) = &patch.preferred_source_id {
        game.preferred_source_id = preferred.clone();
    }
    let was_done = get(conn, id)?.status == GameStatus::Done;
    game.updated_at = now_rfc3339();
    match (was_done, game.status == GameStatus::Done) {
        (false, true) => {
            conn.execute(
                "UPDATE games SET completed_at = ?2 WHERE id = ?1",
                params![id, game.updated_at],
            )?;
        }
        (true, false) => {
            conn.execute("UPDATE games SET completed_at = NULL WHERE id = ?1", [id])?;
        }
        _ => {}
    }
    conn.execute(
        "UPDATE games SET title = ?2, sort_title = ?3, status = ?4, favorite = ?5, hidden = ?6, \
         notes = ?7, preferred_source_id = ?8, updated_at = ?9 WHERE id = ?1",
        params![
            game.id,
            game.title,
            game.sort_title,
            game.status,
            game.favorite,
            game.hidden,
            game.notes,
            game.preferred_source_id,
            game.updated_at
        ],
    )?;
    Ok(game)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    if conn.execute("DELETE FROM games WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound(format!("game {id}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn sort_title_folds_case_accents_and_articles() {
        assert_eq!(sort_title("The Witcher 3"), "witcher 3");
        assert_eq!(sort_title("  Les Sims 4 "), "sims 4");
        assert_eq!(sort_title("L'Amérique"), "amerique");
        assert_eq!(sort_title("Élden Ring"), "elden ring");
        assert_eq!(sort_title("A Plague Tale"), "plague tale");
        // A title that is only an article stays as is.
        assert_eq!(sort_title("The"), "the");
        // "le" is an article only when followed by a space.
        assert_eq!(sort_title("Lethal Company"), "lethal company");
    }

    #[test]
    fn insert_get_list_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let b = insert(&conn, "The Binding of Isaac").unwrap();
        let a = insert(&conn, "Alan Wake 2").unwrap();
        assert_eq!(b.status, GameStatus::Todo);
        assert!(!b.favorite);
        assert_eq!(get(&conn, &a.id).unwrap(), a);
        let titles: Vec<_> = list(&conn, false)
            .unwrap()
            .into_iter()
            .map(|g| g.title)
            .collect();
        assert_eq!(titles, ["Alan Wake 2", "The Binding of Isaac"]);
    }

    #[test]
    fn insert_rejects_empty_title() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(
            insert(&db.conn(), "   "),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn update_applies_only_given_fields() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = insert(&conn, "Hades").unwrap();
        let patch = GamePatch {
            status: Some(GameStatus::Done),
            notes: Some(Some("Fini avec l'épée".into())),
            ..Default::default()
        };
        let updated = update(&conn, &g.id, &patch).unwrap();
        assert_eq!(updated.status, GameStatus::Done);
        assert_eq!(updated.notes.as_deref(), Some("Fini avec l'épée"));
        assert_eq!(updated.title, "Hades");
        assert_eq!(get(&conn, &g.id).unwrap(), updated);

        let clear = GamePatch {
            notes: Some(None),
            ..Default::default()
        };
        let cleared = update(&conn, &g.id, &clear).unwrap();
        assert_eq!(cleared.notes, None);
        assert_eq!(cleared.status, GameStatus::Done);
    }

    #[test]
    fn hidden_games_are_excluded_by_default() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = insert(&conn, "Old demo").unwrap();
        update(
            &conn,
            &g.id,
            &GamePatch {
                hidden: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(list(&conn, false).unwrap().is_empty());
        assert_eq!(list(&conn, true).unwrap().len(), 1);
    }

    #[test]
    fn delete_missing_game_is_not_found() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(
            delete(&db.conn(), "nope"),
            Err(AppError::NotFound(_))
        ));
    }
}
