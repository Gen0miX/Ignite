//! Statistics across all sources (SPEC § 3.5).
//!
//! Time per day comes from two places, combined without double counting:
//! - dated sessions (auto / manual / imported) of every source;
//! - for sources with a platform counter (Steam, GOG), the growth of that
//!   counter between daily snapshots, for the part not already covered by
//!   sessions (time played elsewhere, or while Ludothèque was closed).
//!
//! "Since forever" totals use the cumul rule of the library instead.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::library::query::{self, GameSummary};
use crate::models::{GameStatus, SourceKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StatsRange {
    Week,
    Month,
    Year,
    All,
}

/// Minutes per day for one source: sessions + untracked counter growth.
///
/// `sessions`: minutes of dated sessions per day. `snapshots`: counter value
/// per day (`None` = the source has no platform counter). The counter growth
/// is only added when it exceeds what sessions already explain, cumulatively.
pub fn daily_minutes(
    sessions: &BTreeMap<NaiveDate, f64>,
    snapshots: Option<&BTreeMap<NaiveDate, f64>>,
) -> BTreeMap<NaiveDate, f64> {
    let mut out: BTreeMap<NaiveDate, f64> = sessions.clone();
    let Some(snaps) = snapshots.filter(|s| s.len() > 1) else {
        return out;
    };
    let first = *snaps.keys().next().unwrap();
    let mut deltas: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut prev: Option<f64> = None;
    for (day, value) in snaps {
        if let Some(p) = prev {
            *deltas.entry(*day).or_default() += (value - p).max(0.0);
        }
        prev = Some(*value);
    }
    let days: Vec<NaiveDate> = sessions
        .keys()
        .chain(deltas.keys())
        .copied()
        .filter(|d| *d > first)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let (mut tracked, mut counted, mut assigned) = (0.0, 0.0, 0.0);
    for day in days {
        tracked += sessions.get(&day).copied().unwrap_or(0.0);
        counted += deltas.get(&day).copied().unwrap_or(0.0);
        let extra = (counted - tracked - assigned).max(0.0);
        if extra > 0.0 {
            assigned += extra;
            *out.entry(day).or_default() += extra;
        }
    }
    out
}

/// Longest run of consecutive days with play time: `(days, last day)`.
pub fn longest_streak(days: &BTreeMap<NaiveDate, f64>) -> (u32, Option<NaiveDate>) {
    let (mut best, mut best_end, mut run, mut prev) = (0u32, None, 0u32, None::<NaiveDate>);
    for (day, minutes) in days {
        if *minutes < 1.0 {
            continue;
        }
        run = match prev {
            Some(p) if p.succ_opt() == Some(*day) => run + 1,
            _ => 1,
        };
        prev = Some(*day);
        if run > best {
            best = run;
            best_end = Some(*day);
        }
    }
    (best, best_end)
}

/// First day of the range (inclusive), `None` for "since forever".
pub fn range_start(range: StatsRange, today: NaiveDate) -> Option<NaiveDate> {
    match range {
        StatsRange::Week => {
            Some(today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday())))
        }
        StatsRange::Month => NaiveDate::from_ymd_opt(today.year(), today.month(), 1),
        StatsRange::Year => NaiveDate::from_ymd_opt(today.year(), 1, 1),
        StatsRange::All => None,
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DayMinutes {
    /// YYYY-MM-DD.
    pub day: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TopGame {
    pub game_id: String,
    pub title: String,
    pub cover: Option<String>,
    pub header: Option<String>,
    /// Source with the most time in the range (bar colour).
    pub source: Option<SourceKind>,
    pub minutes: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceMinutes {
    /// `None` = sessions not attached to a source.
    pub kind: Option<SourceKind>,
    pub minutes: u32,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryState {
    pub done: u32,
    pub playing: u32,
    pub dropped: u32,
    pub todo: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    pub range: StatsRange,
    pub total_minutes: u32,
    pub minutes_per_day: u32,
    pub games_played: u32,
    pub games_owned: u32,
    pub games_completed: u32,
    pub achievements_unlocked: u32,
    pub longest_streak_days: u32,
    pub longest_streak_end: Option<String>,
    /// Calendar year shown (current year, or last 12 months for other ranges).
    pub calendar: Vec<DayMinutes>,
    pub top_games: Vec<TopGame>,
    pub by_source: Vec<SourceMinutes>,
    pub library: LibraryState,
    /// First day with dated history (sessions or counter snapshots).
    pub history_start: Option<String>,
    /// Dated sessions (`auto` / `manual`) in the range, with the source filter.
    pub sessions_in_range: u32,
    /// Local day of the first session tracked by Ignite (`auto`), any game.
    pub first_tracked_day: Option<String>,
}

/// `(sessions in the range, first auto session day)`.
fn session_facts(
    conn: &Connection,
    sources: &HashMap<String, SourceInfo>,
    visible: &HashMap<&str, &GameSummary>,
    filter: &[SourceKind],
    in_range: impl Fn(&NaiveDate) -> bool,
) -> AppResult<(u32, Option<NaiveDate>)> {
    let mut stmt = conn.prepare(
        "SELECT game_id, game_source_id, started_at, origin FROM play_sessions \
         WHERE origin IN ('auto', 'manual') AND started_at IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut count = 0;
    let mut first: Option<NaiveDate> = None;
    for row in rows {
        let (game, source, start, origin) = row?;
        let Some(day) = local_day(&start) else {
            continue;
        };
        if origin == "auto" {
            first = Some(first.map_or(day, |f| f.min(day)));
        }
        let kind = source.as_ref().and_then(|s| sources.get(s)).map(|i| i.kind);
        let wanted = filter.is_empty() || kind.is_some_and(|k| filter.contains(&k));
        if visible.contains_key(game.as_str()) && wanted && in_range(&day) {
            count += 1;
        }
    }
    Ok((count, first))
}

fn local_day(rfc3339: &str) -> Option<NaiveDate> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .ok()
        .map(|d| d.with_timezone(&Local).date_naive())
}

fn to_u32(v: f64) -> u32 {
    v.round().clamp(0.0, u32::MAX as f64) as u32
}

struct SourceInfo {
    game_id: String,
    kind: SourceKind,
    counter: bool,
}

/// Per-day minutes for every (game, source) pair; `None` source = orphan sessions.
type Daily = HashMap<(String, Option<String>), BTreeMap<NaiveDate, f64>>;

fn load_daily(conn: &Connection, sources: &HashMap<String, SourceInfo>) -> AppResult<Daily> {
    let mut sessions: Daily = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT game_id, game_source_id, started_at, duration_seconds FROM play_sessions \
         WHERE origin != 'estimate' AND started_at IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (game, source, start, secs) = row?;
        let Some(day) = local_day(&start) else {
            continue;
        };
        let source = source.filter(|s| sources.contains_key(s));
        *sessions
            .entry((game, source))
            .or_default()
            .entry(day)
            .or_default() += secs as f64 / 60.0;
    }

    let mut snaps: HashMap<String, BTreeMap<NaiveDate, f64>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT game_source_id, day, minutes FROM playtime_snapshots")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (source, day, minutes) = row?;
        if let Ok(d) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") {
            snaps.entry(source).or_default().insert(d, minutes as f64);
        }
    }

    let mut daily: Daily = HashMap::new();
    let mut keys: HashSet<(String, Option<String>)> = sessions.keys().cloned().collect();
    for (id, info) in sources {
        if info.counter && snaps.contains_key(id) {
            keys.insert((info.game_id.clone(), Some(id.clone())));
        }
    }
    let empty = BTreeMap::new();
    for key in keys {
        let s = sessions.get(&key).unwrap_or(&empty);
        let counter = key
            .1
            .as_ref()
            .and_then(|id| sources.get(id).filter(|i| i.counter).and(snaps.get(id)));
        daily.insert(key.clone(), daily_minutes(s, counter));
    }
    Ok(daily)
}

pub fn overview(
    conn: &Connection,
    range: StatsRange,
    filter: &[SourceKind],
    today: NaiveDate,
) -> AppResult<StatsOverview> {
    let summaries: Vec<GameSummary> = query::summaries(conn)?
        .into_iter()
        .filter(|g| !g.hidden)
        .collect();
    let visible: HashMap<&str, &GameSummary> =
        summaries.iter().map(|g| (g.id.as_str(), g)).collect();
    let wanted = |k: SourceKind| filter.is_empty() || filter.contains(&k);

    let sources: HashMap<String, SourceInfo> = crate::db::repo::game_sources::list_all(conn)?
        .into_iter()
        .map(|s| {
            (
                s.id.clone(),
                SourceInfo {
                    game_id: s.game_id,
                    kind: s.kind,
                    counter: s.source_playtime_minutes.is_some(),
                },
            )
        })
        .collect();
    let daily = load_daily(conn, &sources)?;
    let start = range_start(range, today);
    let in_range = |d: &NaiveDate| start.is_none_or(|s| *d >= s) && *d <= today;

    // Per-day totals (filtered), per game and per source in the range.
    let mut days: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut per_game: HashMap<String, HashMap<Option<SourceKind>, f64>> = HashMap::new();
    let mut history_start: Option<NaiveDate> = None;
    for ((game, source), values) in &daily {
        if !visible.contains_key(game.as_str()) {
            continue;
        }
        let kind = source.as_ref().and_then(|s| sources.get(s)).map(|i| i.kind);
        if !filter.is_empty() && !kind.is_some_and(wanted) {
            continue;
        }
        for (day, minutes) in values {
            history_start = Some(history_start.map_or(*day, |h: NaiveDate| h.min(*day)));
            *days.entry(*day).or_default() += minutes;
            if in_range(day) {
                *per_game
                    .entry(game.clone())
                    .or_default()
                    .entry(kind)
                    .or_default() += minutes;
            }
        }
    }

    // "Since forever": the library totals (cumul rule), per source.
    if range == StatsRange::All {
        per_game.clear();
        for g in &summaries {
            let detail = crate::library::playtime::breakdown(
                &crate::db::repo::game_sources::list_for_game(conn, &g.id)?
                    .iter()
                    .map(|s| (s.id.clone(), s.source_playtime_minutes))
                    .collect::<Vec<_>>(),
                &crate::db::repo::sessions::list_for_game(conn, &g.id)?
                    .iter()
                    .map(|s| (s.game_source_id.clone(), i64::from(s.duration_seconds)))
                    .collect::<Vec<_>>(),
            );
            for part in detail {
                let kind = part
                    .source_id
                    .as_ref()
                    .and_then(|s| sources.get(s))
                    .map(|i| i.kind);
                if !filter.is_empty() && !kind.is_some_and(wanted) {
                    continue;
                }
                if part.minutes > 0 {
                    *per_game
                        .entry(g.id.clone())
                        .or_default()
                        .entry(kind)
                        .or_default() += part.minutes as f64;
                }
            }
        }
    }

    let (sessions_in_range, first_tracked_day) =
        session_facts(conn, &sources, &visible, filter, in_range)?;

    let total: f64 = per_game.values().flat_map(|m| m.values()).sum();
    let span_days = match start {
        Some(s) => (today - s).num_days() + 1,
        None => history_start.map_or(1, |h| (today - h).num_days() + 1),
    }
    .max(1);

    let mut by_source_map: HashMap<Option<SourceKind>, f64> = HashMap::new();
    for m in per_game.values() {
        for (k, v) in m {
            *by_source_map.entry(*k).or_default() += v;
        }
    }
    let mut by_source: Vec<SourceMinutes> = by_source_map
        .into_iter()
        .filter(|(_, v)| *v >= 1.0)
        .map(|(kind, v)| SourceMinutes {
            kind,
            minutes: to_u32(v),
        })
        .collect();
    by_source.sort_by_key(|s| std::cmp::Reverse(s.minutes));

    let mut top: Vec<TopGame> = per_game
        .iter()
        .filter_map(|(id, m)| {
            let g = visible.get(id.as_str())?;
            let minutes: f64 = m.values().sum();
            let source = m
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .and_then(|(k, _)| *k);
            (minutes >= 1.0).then(|| TopGame {
                game_id: g.id.clone(),
                title: g.title.clone(),
                cover: g.cover.clone(),
                header: g.header.clone(),
                source,
                minutes: to_u32(minutes),
            })
        })
        .collect();
    top.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| a.title.cmp(&b.title))
    });
    let games_played = top.len() as u32;
    top.truncate(8);

    let range_days: BTreeMap<NaiveDate, f64> = days
        .iter()
        .filter(|(d, _)| in_range(d))
        .map(|(d, v)| (*d, *v))
        .collect();
    let (streak, streak_end) = longest_streak(&range_days);

    // Calendar: the current year for "year", otherwise the last 12 months.
    let cal_start = if range == StatsRange::Year {
        NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap()
    } else {
        today - chrono::Duration::days(364)
    };
    let calendar = days
        .range(cal_start..=today)
        .filter(|(_, v)| **v >= 1.0)
        .map(|(d, v)| DayMinutes {
            day: d.to_string(),
            minutes: to_u32(*v),
        })
        .collect();

    let in_range_str = |s: &str| local_day(s).is_some_and(|d| in_range(&d));
    let games_completed = if range == StatsRange::All {
        summaries
            .iter()
            .filter(|g| g.status == GameStatus::Done)
            .count() as u32
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, completed_at FROM games WHERE completed_at IS NOT NULL AND hidden = 0",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut n = 0;
        for row in rows {
            let (id, at) = row?;
            let kinds_ok = visible
                .get(id.as_str())
                .is_some_and(|g| filter.is_empty() || g.sources.iter().any(|k| wanted(*k)));
            if kinds_ok && in_range_str(&at) {
                n += 1;
            }
        }
        n
    };

    let mut stmt = conn.prepare(
        "SELECT s.kind, a.unlocked_at FROM achievements a JOIN game_sources s ON s.id = a.game_source_id \
         WHERE a.unlocked_at IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, SourceKind>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut achievements_unlocked = 0;
    for row in rows {
        let (kind, at) = row?;
        if wanted(kind) && (range == StatsRange::All || in_range_str(&at)) {
            achievements_unlocked += 1;
        }
    }

    let mut library = LibraryState::default();
    for g in summaries
        .iter()
        .filter(|g| filter.is_empty() || g.sources.iter().any(|k| wanted(*k)))
    {
        match g.status {
            GameStatus::Done => library.done += 1,
            GameStatus::Playing => library.playing += 1,
            GameStatus::Dropped => library.dropped += 1,
            GameStatus::Todo => library.todo += 1,
        }
    }
    let games_owned = library.done + library.playing + library.dropped + library.todo;

    Ok(StatsOverview {
        range,
        total_minutes: to_u32(total),
        minutes_per_day: to_u32(total / span_days as f64),
        games_played,
        games_owned,
        games_completed,
        achievements_unlocked,
        longest_streak_days: streak,
        longest_streak_end: streak_end.map(|d| d.to_string()),
        calendar,
        top_games: top,
        by_source,
        library,
        history_start: history_start.map(|d| d.to_string()),
        sessions_in_range,
        first_tracked_day: first_tracked_day.map(|d| d.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::repo::sessions::{self, PlaySession, SessionOrigin};
    use crate::db::repo::{game_sources, games};
    use crate::db::Db;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn map(items: &[(&str, f64)]) -> BTreeMap<NaiveDate, f64> {
        items.iter().map(|(k, v)| (d(k), *v)).collect()
    }

    #[test]
    fn sessions_only_sources_keep_their_sessions() {
        let s = map(&[("2026-11-01", 60.0), ("2026-11-03", 30.0)]);
        assert_eq!(daily_minutes(&s, None), s);
    }

    #[test]
    fn counter_growth_already_covered_by_sessions_is_not_added() {
        // Played 60 min on the 2nd (tracked); the sync on the 3rd sees +60.
        let s = map(&[("2026-11-02", 60.0)]);
        let snaps = map(&[("2026-11-01", 1000.0), ("2026-11-03", 1060.0)]);
        assert_eq!(
            daily_minutes(&s, Some(&snaps)),
            map(&[("2026-11-02", 60.0)])
        );
    }

    #[test]
    fn untracked_counter_growth_is_added_on_the_snapshot_day() {
        // 60 min tracked, but the counter grew by 150: 90 min played elsewhere.
        let s = map(&[("2026-11-02", 60.0)]);
        let snaps = map(&[
            ("2026-11-01", 1000.0),
            ("2026-11-02", 1000.0),
            ("2026-11-04", 1150.0),
        ]);
        assert_eq!(
            daily_minutes(&s, Some(&snaps)),
            map(&[("2026-11-02", 60.0), ("2026-11-04", 90.0)])
        );
        // The first snapshot is a baseline: nothing before it is inferred.
        let only = map(&[("2026-11-01", 1000.0)]);
        assert!(daily_minutes(&BTreeMap::new(), Some(&only)).is_empty());
    }

    #[test]
    fn streaks_need_consecutive_days() {
        let days = map(&[
            ("2026-03-01", 30.0),
            ("2026-03-02", 10.0),
            ("2026-03-03", 5.0),
            ("2026-03-05", 50.0),
            ("2026-03-06", 0.2),
        ]);
        assert_eq!(longest_streak(&days), (3, Some(d("2026-03-03"))));
        assert_eq!(longest_streak(&BTreeMap::new()), (0, None));
    }

    #[test]
    fn ranges_start_on_calendar_boundaries() {
        let today = d("2026-11-06"); // a Friday
        assert_eq!(range_start(StatsRange::Week, today), Some(d("2026-11-02")));
        assert_eq!(range_start(StatsRange::Month, today), Some(d("2026-11-01")));
        assert_eq!(range_start(StatsRange::Year, today), Some(d("2026-01-01")));
        assert_eq!(range_start(StatsRange::All, today), None);
    }

    fn session(id: &str, game: &str, source: &str, start: &str, minutes: u32) -> PlaySession {
        PlaySession {
            id: id.into(),
            game_id: game.into(),
            game_source_id: Some(source.into()),
            started_at: Some(start.into()),
            ended_at: None,
            duration_seconds: minutes * 60,
            idle_seconds: 0,
            origin: SessionOrigin::Auto,
            note: None,
        }
    }

    #[test]
    fn overview_combines_sources_and_respects_the_filter() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let hades = games::insert(&conn, "Hades").unwrap();
        let steam = game_sources::upsert(
            &conn,
            &hades.id,
            SourceKind::Steam,
            &SourceData {
                external_id: Some("1".into()),
                source_playtime_minutes: Some(600),
                ..Default::default()
            },
        )
        .unwrap();
        let celeste = games::insert(&conn, "Celeste").unwrap();
        let local = game_sources::upsert(
            &conn,
            &celeste.id,
            SourceKind::Local,
            &SourceData {
                external_id: Some("c".into()),
                ..Default::default()
            },
        )
        .unwrap();
        sessions::save(
            &conn,
            &session("a", &hades.id, &steam.id, "2026-11-03T18:00:00Z", 90),
        )
        .unwrap();
        sessions::save(
            &conn,
            &session("b", &celeste.id, &local.id, "2026-11-04T18:00:00Z", 45),
        )
        .unwrap();
        sessions::save(
            &conn,
            &session("c", &celeste.id, &local.id, "2026-11-05T18:00:00Z", 15),
        )
        .unwrap();
        sessions::save(
            &conn,
            &session("old", &celeste.id, &local.id, "2025-06-01T18:00:00Z", 100),
        )
        .unwrap();
        games::update(
            &conn,
            &celeste.id,
            &games::GamePatch {
                status: Some(GameStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();

        let today = d("2026-11-06");
        let week = overview(&conn, StatsRange::Week, &[], today).unwrap();
        assert_eq!(week.total_minutes, 150);
        assert_eq!(week.games_played, 2);
        assert_eq!(week.top_games[0].title, "Hades");
        assert_eq!(week.top_games[0].source, Some(SourceKind::Steam));
        assert_eq!(week.longest_streak_days, 3);
        assert_eq!(week.games_completed, 1);
        assert_eq!(week.minutes_per_day, 30); // 150 min over Mon..Fri
        assert_eq!(week.sessions_in_range, 3);
        assert_eq!(week.first_tracked_day.as_deref(), Some("2025-06-01"));

        let steam_only = overview(&conn, StatsRange::Week, &[SourceKind::Steam], today).unwrap();
        assert_eq!(steam_only.total_minutes, 90);
        assert_eq!(steam_only.by_source.len(), 1);
        assert_eq!(steam_only.sessions_in_range, 1);

        // Since forever: Steam counter (600) + Celeste sessions (160).
        let all = overview(&conn, StatsRange::All, &[], today).unwrap();
        assert_eq!(all.total_minutes, 760);
        assert_eq!(all.library.done, 1);
        assert_eq!(all.games_owned, 2);
        assert_eq!(all.history_start.as_deref(), Some("2025-06-01"));
        assert_eq!(all.sessions_in_range, 4);
    }
}
