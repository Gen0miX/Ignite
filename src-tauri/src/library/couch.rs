//! Couch mode (SPEC § 3.8): shelves for the gamepad carousel and the random wheel.

use rusqlite::Connection;
use serde::Deserialize;

use crate::error::AppResult;
use crate::library::home::{home_game, HomeGame};
use crate::library::query::{self, Context, GameSummary};
use crate::library::rules::Facts;

/// Carousel shelves, switched with LB / RB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Shelf {
    Recent,
    Favorites,
    Coop,
    Short,
    All,
}

/// Moods of the random wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Mood {
    Chill,
    Intense,
    Coop,
    Never,
}

const CHILL_GENRES: &[&str] = &[
    "Simulation",
    "Réflexion",
    "Occasionnel",
    "Aventure",
    "Point & click",
    "Visual novel",
    "Cartes & plateau",
    "Musique",
];
const INTENSE_GENRES: &[&str] = &[
    "Action",
    "Tir",
    "Combat",
    "Beat'em all",
    "Course",
    "Sport",
    "Arcade",
];
/// Sessions this short or shorter make a "short session" game.
const SHORT_SESSION_MINUTES: f64 = 45.0;
const SHORT_GENRES: &[&str] = &["Arcade", "Combat", "Course", "Sport", "Occasionnel"];
const SHELF_LIMIT: usize = 40;

fn has_any(list: &[String], wanted: &[&str]) -> bool {
    list.iter().any(|g| wanted.contains(&g.as_str()))
}

pub fn is_coop(f: &Facts) -> bool {
    f.modes
        .iter()
        .any(|m| m == "Coopération" || m == "Écran partagé")
}

pub fn in_shelf(f: &Facts, shelf: Shelf) -> bool {
    match shelf {
        Shelf::Recent => f.last_played.is_some(),
        Shelf::Favorites => f.favorite,
        Shelf::Coop => f.installed && is_coop(f),
        Shelf::Short => {
            f.installed
                && match f.avg_session_minutes {
                    Some(avg) => avg <= SHORT_SESSION_MINUTES,
                    None => has_any(&f.genres, SHORT_GENRES),
                }
        }
        Shelf::All => true,
    }
}

/// Whether the wheel may land on this game for this mood. The wheel draws from
/// the pile: installed games that are neither finished nor dropped.
pub fn wheel_matches(f: &Facts, mood: Mood) -> bool {
    if !f.installed || f.status == "done" || f.status == "dropped" {
        return false;
    }
    match mood {
        Mood::Chill => has_any(&f.genres, CHILL_GENRES) && !has_any(&f.genres, &["Tir", "Combat"]),
        Mood::Intense => has_any(&f.genres, INTENSE_GENRES),
        Mood::Coop => is_coop(f),
        Mood::Never => f.playtime_minutes == 0.0 && f.last_played.is_none(),
    }
}

fn visible(conn: &Connection) -> AppResult<(Vec<GameSummary>, Context)> {
    let list: Vec<GameSummary> = query::summaries(conn)?
        .into_iter()
        .filter(|g| !g.hidden)
        .collect();
    let ctx = Context::load(conn, &list)?;
    Ok((list, ctx))
}

pub fn shelf(conn: &Connection, shelf: Shelf) -> AppResult<Vec<HomeGame>> {
    let (list, ctx) = visible(conn)?;
    let mut games: Vec<GameSummary> = list
        .into_iter()
        .filter(|g| ctx.facts.get(&g.id).is_some_and(|f| in_shelf(f, shelf)))
        .collect();
    match shelf {
        Shelf::Recent => games.sort_by(|a, b| b.last_played.cmp(&a.last_played)),
        Shelf::All => games.sort_by(|a, b| a.sort_title.cmp(&b.sort_title)),
        _ => games.sort_by(|a, b| {
            b.last_played
                .cmp(&a.last_played)
                .then_with(|| a.sort_title.cmp(&b.sort_title))
        }),
    }
    let limit = if shelf == Shelf::All {
        usize::MAX
    } else {
        SHELF_LIMIT
    };
    Ok(games
        .into_iter()
        .take(limit)
        .map(|g| home_game(g, &ctx))
        .collect())
}

/// Every game the wheel may land on; the frontend draws one.
pub fn wheel(conn: &Connection, mood: Mood) -> AppResult<Vec<HomeGame>> {
    let (list, ctx) = visible(conn)?;
    Ok(list
        .into_iter()
        .filter(|g| ctx.facts.get(&g.id).is_some_and(|f| wheel_matches(f, mood)))
        .map(|g| home_game(g, &ctx))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(genres: &[&str], modes: &[&str]) -> Facts {
        Facts {
            title: "Jeu".into(),
            tags: vec![],
            genres: genres.iter().map(|s| s.to_string()).collect(),
            modes: modes.iter().map(|s| s.to_string()).collect(),
            sources: vec!["steam".into()],
            installed: true,
            status: "todo".into(),
            favorite: false,
            playtime_minutes: 120.0,
            avg_session_minutes: None,
            last_played: Some(chrono::Utc::now()),
            achievement_percent: None,
            hltb_main_minutes: None,
            added_at: None,
        }
    }

    #[test]
    fn wheel_moods_follow_genres_and_modes() {
        let sim = facts(&["Simulation"], &["Solo"]);
        let shooter = facts(&["Action", "Tir"], &["Coopération"]);
        assert!(wheel_matches(&sim, Mood::Chill));
        assert!(!wheel_matches(&sim, Mood::Intense));
        assert!(wheel_matches(&shooter, Mood::Intense));
        assert!(wheel_matches(&shooter, Mood::Coop));
        assert!(!wheel_matches(&sim, Mood::Coop));
        // A shooter with an adventure tag is not chill.
        let mixed = facts(&["Aventure", "Tir"], &[]);
        assert!(!wheel_matches(&mixed, Mood::Chill));
    }

    #[test]
    fn wheel_draws_from_the_installed_pile_only() {
        let never = Facts {
            playtime_minutes: 0.0,
            last_played: None,
            ..facts(&[], &[])
        };
        assert!(wheel_matches(&never, Mood::Never));
        assert!(!wheel_matches(&facts(&[], &[]), Mood::Never));
        let done = Facts {
            status: "done".into(),
            ..never.clone()
        };
        assert!(!wheel_matches(&done, Mood::Never));
        let missing = Facts {
            installed: false,
            ..never
        };
        assert!(!wheel_matches(&missing, Mood::Never));
    }

    #[test]
    fn short_shelf_prefers_measured_sessions() {
        let quick = Facts {
            avg_session_minutes: Some(20.0),
            ..facts(&["RPG"], &[])
        };
        let long_fighter = Facts {
            avg_session_minutes: Some(120.0),
            ..facts(&["Combat"], &[])
        };
        let unmeasured_fighter = facts(&["Combat"], &[]);
        assert!(in_shelf(&quick, Shelf::Short));
        assert!(!in_shelf(&long_fighter, Shelf::Short));
        assert!(in_shelf(&unmeasured_fighter, Shelf::Short));
        assert!(!in_shelf(&facts(&["RPG"], &[]), Shelf::Short));
    }

    #[test]
    fn coop_shelf_needs_an_installed_coop_game() {
        let coop = facts(&[], &["Écran partagé"]);
        assert!(in_shelf(&coop, Shelf::Coop));
        let gone = Facts {
            installed: false,
            ..coop
        };
        assert!(!in_shelf(&gone, Shelf::Coop));
    }
}
