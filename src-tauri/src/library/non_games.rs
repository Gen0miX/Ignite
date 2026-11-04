//! Entries a platform lists as "games" that are not really part of the
//! library: playtests, betas, demos, test servers... They are imported but
//! hidden; the user can show them again from the "Jeux masqués" view.

use rusqlite::{params, Connection};

use crate::db::repo::{games, settings};
use crate::error::AppResult;

/// Lowercase words, split on anything that is not a letter or digit.
fn words(title: &str) -> Vec<String> {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// True for titles such as "Arkheron Playtest", "Battlefield 6 Open Beta",
/// "Half Sword Demo", "H1Z1: Test Server". Whole words only, so
/// "Demolition" or "Alphabet" are kept.
pub fn is_probably_not_a_game(title: &str) -> bool {
    const WORDS: &[&str] = &["playtest", "demo", "beta", "benchmark", "soundtrack", "sdk"];
    const PAIRS: &[(&str, &str)] = &[
        ("test", "server"),
        ("public", "test"),
        ("dedicated", "server"),
        ("closed", "alpha"),
    ];
    let w = words(title);
    w.iter().any(|x| WORDS.contains(&x.as_str()))
        || w.windows(2)
            .any(|p| PAIRS.contains(&(p[0].as_str(), p[1].as_str())))
}

const APPLIED_KEY: &str = "library.auto_hide_non_games_v1";

/// Hides already-imported non-games, once. Later imports hide new entries at
/// creation, so a game the user shows again is never hidden a second time.
pub fn hide_existing_once(conn: &Connection) -> AppResult<u32> {
    if settings::get::<bool>(conn, APPLIED_KEY)?.unwrap_or(false) {
        return Ok(0);
    }
    let mut hidden = 0;
    for game in games::list(conn, false)? {
        if is_probably_not_a_game(&game.title) {
            conn.execute(
                "UPDATE games SET hidden = 1, updated_at = ?2 WHERE id = ?1",
                params![game.id, crate::db::now_rfc3339()],
            )?;
            hidden += 1;
        }
    }
    settings::set(conn, APPLIED_KEY, &true)?;
    Ok(hidden)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games::GamePatch;
    use crate::db::Db;

    #[test]
    fn detects_playtests_betas_demos_and_servers() {
        for t in [
            "Arkheron Playtest",
            "Battlefield™ 6 Open Beta",
            "Monster Hunter Wilds Beta test",
            "Half Sword Demo",
            "H1Z1: Test Server",
            "Steep Open Beta",
            "Throne and Liberty - Open Beta",
            "Some Game - Public Test",
            "ARK Dedicated Server",
            "Hades II Soundtrack",
        ] {
            assert!(is_probably_not_a_game(t), "{t}");
        }
    }

    #[test]
    fn keeps_real_games() {
        for t in [
            "Demolition Company",
            "Demon's Souls",
            "Alphabeta",
            "Betrayal at House on the Hill",
            "Testament of Sherlock Holmes",
            "Final Fantasy VII Remake Intergrade",
            "Half Sword",
            "Life is Strange: Before the Storm - Prologue",
            "Server Wars",
        ] {
            assert!(!is_probably_not_a_game(t), "{t}");
        }
    }

    #[test]
    fn hides_existing_entries_only_once() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let beta = games::insert(&conn, "Steep Open Beta").unwrap();
        games::insert(&conn, "Steep").unwrap();
        assert_eq!(hide_existing_once(&conn).unwrap(), 1);
        assert!(games::get(&conn, &beta.id).unwrap().hidden);

        // The user shows it again: a second run must not hide it back.
        games::update(
            &conn,
            &beta.id,
            &GamePatch {
                hidden: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(hide_existing_once(&conn).unwrap(), 0);
        assert!(!games::get(&conn, &beta.id).unwrap().hidden);
    }
}
