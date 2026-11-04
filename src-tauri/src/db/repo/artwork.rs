use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq)]
pub struct ArtworkRow {
    pub game_id: String,
    pub kind: String,
    pub local_path: String,
    pub provider: Option<String>,
    pub source_url: Option<String>,
}

pub fn upsert(conn: &Connection, row: &ArtworkRow) -> AppResult<()> {
    conn.execute(
        "INSERT INTO artwork (game_id, kind, local_path, provider, source_url) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(game_id, kind) DO UPDATE SET local_path = excluded.local_path, \
           provider = excluded.provider, source_url = excluded.source_url",
        params![
            row.game_id,
            row.kind,
            row.local_path,
            row.provider,
            row.source_url
        ],
    )?;
    Ok(())
}

/// `kind → local_path` for one game.
pub fn for_game(conn: &Connection, game_id: &str) -> AppResult<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT kind, local_path FROM artwork WHERE game_id = ?1")?;
    let rows = stmt.query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// `game_id → (kind → local_path)` for every game.
pub fn all(conn: &Connection) -> AppResult<HashMap<String, HashMap<String, String>>> {
    let mut stmt = conn.prepare("SELECT game_id, kind, local_path FROM artwork")?;
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (game, kind, path) = row?;
        map.entry(game).or_default().insert(kind, path);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    #[test]
    fn upsert_and_read_back() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "A").unwrap();
        let mut row = ArtworkRow {
            game_id: g.id.clone(),
            kind: "header".into(),
            local_path: "C:\\a.jpg".into(),
            provider: Some("steam".into()),
            source_url: None,
        };
        upsert(&conn, &row).unwrap();
        row.local_path = "C:\\b.jpg".into();
        upsert(&conn, &row).unwrap();
        assert_eq!(for_game(&conn, &g.id).unwrap()["header"], "C:\\b.jpg");
        assert_eq!(all(&conn).unwrap()[&g.id].len(), 1);
    }
}
