//! Time to beat per game and provider (docs/TIME-TO-BEAT.md). The manual
//! correction always wins, field by field, and is never overwritten by a fetch.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::now_rfc3339;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum TtbProvider {
    Manual,
    Igdb,
}

impl TtbProvider {
    fn as_str(self) -> &'static str {
        match self {
            TtbProvider::Manual => "manual",
            TtbProvider::Igdb => "igdb",
        }
    }

    fn parse(s: &str) -> rusqlite::Result<Self> {
        match s {
            "manual" => Ok(TtbProvider::Manual),
            "igdb" => Ok(TtbProvider::Igdb),
            other => Err(rusqlite::Error::InvalidColumnType(
                1,
                format!("provider {other}"),
                rusqlite::types::Type::Text,
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TimeToBeat {
    pub provider: TtbProvider,
    pub external_id: Option<String>,
    /// "Histoire".
    pub main_minutes: Option<u32>,
    /// "Histoire + extras".
    pub extra_minutes: Option<u32>,
    /// "Complétionniste".
    pub completionist_minutes: Option<u32>,
    /// Players behind the values, when the provider says.
    pub sample_count: Option<u32>,
    pub url: Option<String>,
    pub fetched_at: String,
}

impl TimeToBeat {
    pub fn is_empty(&self) -> bool {
        self.main_minutes.is_none()
            && self.extra_minutes.is_none()
            && self.completionist_minutes.is_none()
    }
}

const COLUMNS: &str = "provider, external_id, main_minutes, extra_minutes, completionist_minutes, \
     sample_count, url, fetched_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<TimeToBeat> {
    let minutes = |v: Option<i64>| v.map(|m| m.clamp(0, u32::MAX as i64) as u32);
    Ok(TimeToBeat {
        provider: TtbProvider::parse(&r.get::<_, String>(0)?)?,
        external_id: r.get(1)?,
        main_minutes: minutes(r.get(2)?),
        extra_minutes: minutes(r.get(3)?),
        completionist_minutes: minutes(r.get(4)?),
        sample_count: minutes(r.get(5)?),
        url: r.get(6)?,
        fetched_at: r.get(7)?,
    })
}

/// Rows of a game, manual first.
pub fn list(conn: &Connection, game_id: &str) -> AppResult<Vec<TimeToBeat>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM game_time_to_beat WHERE game_id = ?1 \
         ORDER BY CASE provider WHEN 'manual' THEN 0 ELSE 1 END"
    ))?;
    let rows = stmt.query_map([game_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(
    conn: &Connection,
    game_id: &str,
    provider: TtbProvider,
) -> AppResult<Option<TimeToBeat>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM game_time_to_beat WHERE game_id = ?1 AND provider = ?2"
            ),
            params![game_id, provider.as_str()],
            from_row,
        )
        .optional()?)
}

/// Best value per field: the manual correction where set, else IGDB.
pub fn best(rows: &[TimeToBeat]) -> Option<TimeToBeat> {
    let manual = rows.iter().find(|r| r.provider == TtbProvider::Manual);
    let fetched = rows
        .iter()
        .find(|r| r.provider != TtbProvider::Manual && !r.is_empty());
    match (manual, fetched) {
        (None, None) => None,
        (Some(m), None) => Some(m.clone()),
        (None, Some(f)) => Some(f.clone()),
        (Some(m), Some(f)) => Some(TimeToBeat {
            main_minutes: m.main_minutes.or(f.main_minutes),
            extra_minutes: m.extra_minutes.or(f.extra_minutes),
            completionist_minutes: m.completionist_minutes.or(f.completionist_minutes),
            ..m.clone()
        }),
    }
}

/// Keeps `games.hltb_main_minutes` (library filters, smart collections, home)
/// in line with the best "main story" value.
fn refresh_main(conn: &Connection, game_id: &str) -> AppResult<()> {
    let main = best(&list(conn, game_id)?).and_then(|b| b.main_minutes);
    conn.execute(
        "UPDATE games SET hltb_main_minutes = ?2 WHERE id = ?1",
        params![game_id, main],
    )?;
    Ok(())
}

/// Writes a fetched value (never the manual row) and refreshes the best one.
pub fn save_fetched(conn: &Connection, game_id: &str, row: &TimeToBeat) -> AppResult<()> {
    if row.provider == TtbProvider::Manual {
        return Err(AppError::Invalid("use set_manual for corrections".into()));
    }
    conn.execute(
        "INSERT INTO game_time_to_beat (game_id, provider, external_id, main_minutes, extra_minutes, \
           completionist_minutes, sample_count, url, fetched_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
         ON CONFLICT(game_id, provider) DO UPDATE SET external_id = excluded.external_id, \
           main_minutes = excluded.main_minutes, extra_minutes = excluded.extra_minutes, \
           completionist_minutes = excluded.completionist_minutes, \
           sample_count = excluded.sample_count, url = excluded.url, fetched_at = excluded.fetched_at",
        params![
            game_id,
            row.provider.as_str(),
            row.external_id,
            row.main_minutes,
            row.extra_minutes,
            row.completionist_minutes,
            row.sample_count,
            row.url,
            row.fetched_at
        ],
    )?;
    refresh_main(conn, game_id)
}

/// Longest correction accepted: 10 000 h.
const MAX_MINUTES: u32 = 600_000;

/// Sets the user's correction; `None` (or all fields empty) removes it.
pub fn set_manual(
    conn: &Connection,
    game_id: &str,
    values: Option<(Option<u32>, Option<u32>, Option<u32>)>,
) -> AppResult<()> {
    let values = values.filter(|(a, b, c)| a.is_some() || b.is_some() || c.is_some());
    match values {
        None => {
            conn.execute(
                "DELETE FROM game_time_to_beat WHERE game_id = ?1 AND provider = 'manual'",
                [game_id],
            )?;
        }
        Some((main, extra, completionist)) => {
            if [main, extra, completionist]
                .iter()
                .flatten()
                .any(|m| *m == 0 || *m > MAX_MINUTES)
            {
                return Err(AppError::Invalid("time to beat out of range".into()));
            }
            conn.execute(
                "INSERT INTO game_time_to_beat (game_id, provider, main_minutes, extra_minutes, \
                   completionist_minutes, fetched_at) VALUES (?1, 'manual', ?2, ?3, ?4, ?5) \
                 ON CONFLICT(game_id, provider) DO UPDATE SET main_minutes = excluded.main_minutes, \
                   extra_minutes = excluded.extra_minutes, \
                   completionist_minutes = excluded.completionist_minutes, \
                   fetched_at = excluded.fetched_at",
                params![game_id, main, extra, completionist, now_rfc3339()],
            )?;
        }
    }
    refresh_main(conn, game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{repo::games, Db};

    fn igdb(main: u32, extra: u32, done: u32) -> TimeToBeat {
        TimeToBeat {
            provider: TtbProvider::Igdb,
            external_id: Some("113112".into()),
            main_minutes: Some(main),
            extra_minutes: Some(extra),
            completionist_minutes: Some(done),
            sample_count: Some(13),
            url: Some("https://www.igdb.com/games/hades--1".into()),
            fetched_at: "2026-11-08T10:00:00Z".into(),
        }
    }

    fn hltb_column(conn: &Connection, id: &str) -> Option<i64> {
        conn.query_row(
            "SELECT hltb_main_minutes FROM games WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn manual_correction_wins_field_by_field_and_is_never_overwritten() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();

        save_fetched(&conn, &g.id, &igdb(1320, 2100, 4200)).unwrap();
        assert_eq!(hltb_column(&conn, &g.id), Some(1320));

        set_manual(&conn, &g.id, Some((Some(1500), None, None))).unwrap();
        // A new fetch does not touch the correction.
        save_fetched(&conn, &g.id, &igdb(1200, 2000, 4000)).unwrap();
        let rows = list(&conn, &g.id).unwrap();
        assert_eq!(rows.len(), 2);
        let b = best(&rows).unwrap();
        assert_eq!(b.provider, TtbProvider::Manual);
        assert_eq!(
            (b.main_minutes, b.extra_minutes, b.completionist_minutes),
            (Some(1500), Some(2000), Some(4000))
        );
        assert_eq!(hltb_column(&conn, &g.id), Some(1500));

        // Removing the correction falls back on IGDB.
        set_manual(&conn, &g.id, None).unwrap();
        assert_eq!(hltb_column(&conn, &g.id), Some(1200));
        assert!(get(&conn, &g.id, TtbProvider::Manual).unwrap().is_none());

        assert!(set_manual(&conn, &g.id, Some((Some(0), None, None))).is_err());
        assert!(save_fetched(
            &conn,
            &g.id,
            &TimeToBeat {
                provider: TtbProvider::Manual,
                ..igdb(1, 1, 1)
            }
        )
        .is_err());
    }

    #[test]
    fn empty_fetch_is_not_a_best_value() {
        let empty = TimeToBeat {
            main_minutes: None,
            extra_minutes: None,
            completionist_minutes: None,
            ..igdb(1, 1, 1)
        };
        assert_eq!(best(&[empty]), None);
    }
}
