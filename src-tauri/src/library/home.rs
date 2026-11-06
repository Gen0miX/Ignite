//! Home screen (SPEC § 3.7): resume, "Ce soir, j'ai…", pile to finish, week.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::library::query::{self, Context, GameSummary};
use crate::models::GameStatus;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HomeGame {
    pub game: GameSummary,
    pub avg_session_minutes: Option<u32>,
}

/// Time the user has tonight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Tonight {
    HalfHour,
    Hour,
    Evening,
    Coop,
}

#[derive(Debug, Clone, Default)]
pub struct Candidate {
    pub installed: bool,
    pub status: Option<GameStatus>,
    pub playtime_minutes: u32,
    pub avg_session_minutes: Option<f64>,
    pub last_played: Option<DateTime<Utc>>,
    pub coop: bool,
}

/// Score of a game for tonight; `None` = not suggested.
pub fn tonight_score(c: &Candidate, choice: Tonight, now: DateTime<Utc>) -> Option<f64> {
    if !c.installed || matches!(c.status, Some(GameStatus::Done) | Some(GameStatus::Dropped)) {
        return None;
    }
    let budget = match choice {
        Tonight::HalfHour => 30.0,
        Tonight::Hour => 60.0,
        Tonight::Evening => 240.0,
        Tonight::Coop => 240.0,
    };
    if choice == Tonight::Coop && !c.coop {
        return None;
    }
    let mut score = 0.0;
    score += match c.status {
        Some(GameStatus::Playing) => 3.0,
        _ if c.playtime_minutes > 0 => 1.0,
        _ => 0.5,
    };
    match c.avg_session_minutes {
        // Usual sessions fit in the time available.
        Some(avg) if avg <= budget * 1.2 => score += 2.0,
        // Long-session games are a bad fit for a short slot.
        Some(_) if budget < 120.0 => return None,
        Some(_) => score += 1.0,
        None => score += if budget >= 120.0 { 0.5 } else { 0.0 },
    }
    if let Some(last) = c.last_played {
        let days = (now - last).num_days();
        if days <= 14 {
            score += 1.5;
        } else if days > 365 {
            score += 0.3; // rediscover an old favourite
        }
    }
    Some(score)
}

fn avg_minutes(ctx: &Context, id: &str) -> Option<f64> {
    ctx.facts.get(id).and_then(|f| f.avg_session_minutes)
}

fn candidate(g: &GameSummary, ctx: &Context) -> Candidate {
    let facts = ctx.facts.get(&g.id);
    Candidate {
        installed: g.installed,
        status: Some(g.status),
        playtime_minutes: g.playtime_minutes,
        avg_session_minutes: avg_minutes(ctx, &g.id),
        last_played: facts.and_then(|f| f.last_played),
        coop: facts.is_some_and(|f| {
            f.modes
                .iter()
                .any(|m| m == "Coopération" || m == "Écran partagé")
        }),
    }
}

pub fn home_game(g: GameSummary, ctx: &Context) -> HomeGame {
    let avg = avg_minutes(ctx, &g.id).map(|m| m.round() as u32);
    HomeGame {
        game: g,
        avg_session_minutes: avg,
    }
}

pub fn tonight(conn: &Connection, choice: Tonight, now: DateTime<Utc>) -> AppResult<Vec<HomeGame>> {
    let list: Vec<GameSummary> = query::summaries(conn)?
        .into_iter()
        .filter(|g| !g.hidden)
        .collect();
    let ctx = Context::load(conn, &list)?;
    let mut scored: Vec<(f64, GameSummary)> = list
        .into_iter()
        .filter_map(|g| tonight_score(&candidate(&g, &ctx), choice, now).map(|s| (s, g)))
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.sort_title.cmp(&b.1.sort_title))
    });
    Ok(scored
        .into_iter()
        .take(8)
        .map(|(_, g)| home_game(g, &ctx))
        .collect())
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HomeOverview {
    /// Most recently played game.
    pub resume: Option<HomeGame>,
    pub recent: Vec<HomeGame>,
    /// Never launched or dropped games, installed first.
    pub pile: Vec<HomeGame>,
    pub pile_total: u32,
}

pub fn overview(conn: &Connection) -> AppResult<HomeOverview> {
    let list: Vec<GameSummary> = query::summaries(conn)?
        .into_iter()
        .filter(|g| !g.hidden)
        .collect();
    let ctx = Context::load(conn, &list)?;
    let mut played: Vec<&GameSummary> = list.iter().filter(|g| g.last_played.is_some()).collect();
    played.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    let resume = played.first().map(|g| home_game((*g).clone(), &ctx));
    let recent = played
        .iter()
        .skip(1)
        .take(6)
        .map(|g| home_game((*g).clone(), &ctx))
        .collect();

    let mut pile: Vec<&GameSummary> = list
        .iter()
        .filter(|g| {
            g.status == GameStatus::Dropped
                || (g.status == GameStatus::Todo && g.playtime_minutes == 0)
        })
        .collect();
    pile.sort_by(|a, b| {
        b.installed
            .cmp(&a.installed)
            .then(b.added_at.cmp(&a.added_at))
            .then(a.sort_title.cmp(&b.sort_title))
    });
    let pile_total = pile.len() as u32;
    Ok(HomeOverview {
        resume,
        recent,
        pile: pile
            .into_iter()
            .take(8)
            .map(|g| home_game(g.clone(), &ctx))
            .collect(),
        pile_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 6, 20, 0, 0).unwrap()
    }

    fn c(status: GameStatus, avg: Option<f64>, days_ago: Option<i64>) -> Candidate {
        Candidate {
            installed: true,
            status: Some(status),
            playtime_minutes: if avg.is_some() { 300 } else { 0 },
            avg_session_minutes: avg,
            last_played: days_ago.map(|d| now() - chrono::Duration::days(d)),
            coop: false,
        }
    }

    #[test]
    fn short_slots_exclude_long_session_games() {
        let short = c(GameStatus::Playing, Some(25.0), Some(2));
        let long = c(GameStatus::Playing, Some(150.0), Some(2));
        assert!(tonight_score(&short, Tonight::HalfHour, now()).is_some());
        assert!(tonight_score(&long, Tonight::HalfHour, now()).is_none());
        assert!(tonight_score(&long, Tonight::Evening, now()).is_some());
    }

    #[test]
    fn games_in_progress_and_recent_ones_come_first() {
        let playing = tonight_score(
            &c(GameStatus::Playing, Some(50.0), Some(3)),
            Tonight::Hour,
            now(),
        )
        .unwrap();
        let backlog = tonight_score(
            &c(GameStatus::Todo, Some(50.0), Some(200)),
            Tonight::Hour,
            now(),
        )
        .unwrap();
        assert!(playing > backlog);
    }

    #[test]
    fn finished_dropped_uninstalled_and_non_coop_are_excluded() {
        assert!(tonight_score(
            &c(GameStatus::Done, Some(30.0), None),
            Tonight::Evening,
            now()
        )
        .is_none());
        assert!(tonight_score(
            &c(GameStatus::Dropped, Some(30.0), None),
            Tonight::Evening,
            now()
        )
        .is_none());
        let not_installed = Candidate {
            installed: false,
            ..c(GameStatus::Playing, Some(30.0), None)
        };
        assert!(tonight_score(&not_installed, Tonight::Evening, now()).is_none());
        let solo = c(GameStatus::Playing, Some(30.0), None);
        assert!(tonight_score(&solo, Tonight::Coop, now()).is_none());
        let coop = Candidate { coop: true, ..solo };
        assert!(tonight_score(&coop, Tonight::Coop, now()).is_some());
    }
}
