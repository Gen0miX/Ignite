//! End-of-session recap (docs/SESSION-RECAP.md): pure helpers for the
//! timeline, the streak of days, the achievements unlocked during the session
//! and the average session, tested without a database.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde::Serialize;

use crate::db::repo::achievements::AchievementRow;

/// Part of a session: played (`idle = false`) or idle time removed.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSegment {
    pub start: String,
    pub end: String,
    pub idle: bool,
}

fn rfc3339(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Splits `[start, end]` into active and idle segments. Idle stretches are
/// clipped to the session, sorted and merged when they overlap; without any,
/// the timeline is one active segment.
pub fn timeline(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    idle: &[(DateTime<Utc>, DateTime<Utc>)],
) -> Vec<TimelineSegment> {
    if end <= start {
        return vec![];
    }
    let mut stretches: Vec<(DateTime<Utc>, DateTime<Utc>)> = idle
        .iter()
        .map(|(a, b)| ((*a).max(start), (*b).min(end)))
        .filter(|(a, b)| a < b)
        .collect();
    stretches.sort();
    let mut merged: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
    for (a, b) in stretches {
        match merged.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    let mut out = Vec::new();
    let mut cursor = start;
    for (a, b) in merged {
        if a > cursor {
            out.push(TimelineSegment {
                start: rfc3339(cursor),
                end: rfc3339(a),
                idle: false,
            });
        }
        out.push(TimelineSegment {
            start: rfc3339(a),
            end: rfc3339(b),
            idle: true,
        });
        cursor = b;
    }
    if cursor < end {
        out.push(TimelineSegment {
            start: rfc3339(cursor),
            end: rfc3339(end),
            idle: false,
        });
    }
    out
}

/// Local calendar days a session touches (both, when it crosses midnight).
pub fn session_days(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<NaiveDate> {
    let first = start.with_timezone(&Local).date_naive();
    let last = end.max(start).with_timezone(&Local).date_naive();
    let mut days = vec![first];
    let mut d = first;
    while d < last {
        d = d.succ_opt().unwrap_or(last);
        days.push(d);
    }
    days
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
pub struct Streak {
    /// Consecutive days with a session, up to `today`.
    pub current: u32,
    pub best: u32,
}

pub fn streak(days: &BTreeSet<NaiveDate>, today: NaiveDate) -> Streak {
    let mut best = 0;
    let mut run = 0;
    let mut prev: Option<NaiveDate> = None;
    for d in days {
        run = match prev {
            Some(p) if *d == p + Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(*d);
    }
    let mut current = 0;
    let mut d = today;
    while days.contains(&d) {
        current += 1;
        d -= Duration::days(1);
    }
    Streak { current, best }
}

/// Unlocks may be stamped a little after the game closed (sync delay).
const UNLOCK_GRACE_SECS: i64 = 120;

/// Achievements unlocked between `start` and `end` (plus a short grace),
/// oldest first.
pub fn unlocked_during(
    rows: &[AchievementRow],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Vec<AchievementRow> {
    let until = end + Duration::seconds(UNLOCK_GRACE_SECS);
    let mut out: Vec<(DateTime<Utc>, AchievementRow)> = rows
        .iter()
        .filter_map(|r| {
            let at = DateTime::parse_from_rfc3339(r.unlocked_at.as_deref()?)
                .ok()?
                .with_timezone(&Utc);
            (at >= start && at <= until).then(|| (at, r.clone()))
        })
        .collect();
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, r)| r).collect()
}

/// Average of the other sessions; `None` under 3 of them (not meaningful).
pub fn average_seconds(others: &[u32]) -> Option<u32> {
    if others.len() < 3 {
        return None;
    }
    let sum: u64 = others.iter().map(|s| u64::from(*s)).sum();
    Some((sum / others.len() as u64) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 6, h, m, 0).unwrap()
    }

    #[test]
    fn timeline_without_pause_is_one_bar() {
        let t = timeline(at(19, 12), at(21, 36), &[]);
        assert_eq!(t.len(), 1);
        assert!(!t[0].idle);
        assert_eq!(t[0].start, "2026-11-06T19:12:00Z");
        assert_eq!(t[0].end, "2026-11-06T21:36:00Z");
    }

    #[test]
    fn timeline_splits_clips_and_merges_pauses() {
        let t = timeline(
            at(19, 0),
            at(22, 0),
            &[
                (at(20, 4), at(20, 12)),
                (at(20, 10), at(20, 20)), // overlaps the first one
                (at(18, 0), at(19, 10)),  // starts before the session
                (at(23, 0), at(23, 30)),  // after the session: dropped
            ],
        );
        let kinds: Vec<_> = t
            .iter()
            .map(|s| (s.start.as_str(), s.end.as_str(), s.idle))
            .collect();
        assert_eq!(
            kinds,
            [
                ("2026-11-06T19:00:00Z", "2026-11-06T19:10:00Z", true),
                ("2026-11-06T19:10:00Z", "2026-11-06T20:04:00Z", false),
                ("2026-11-06T20:04:00Z", "2026-11-06T20:20:00Z", true),
                ("2026-11-06T20:20:00Z", "2026-11-06T22:00:00Z", false),
            ]
        );
        assert!(timeline(at(20, 0), at(20, 0), &[]).is_empty());
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn streak_counts_local_days_up_to_today() {
        let days: BTreeSet<_> = [
            "2026-10-01",
            "2026-10-02",
            "2026-10-03",
            "2026-10-04",
            "2026-11-04",
            "2026-11-05",
            "2026-11-06",
        ]
        .iter()
        .map(|s| d(s))
        .collect();
        assert_eq!(
            streak(&days, d("2026-11-06")),
            Streak {
                current: 3,
                best: 4
            }
        );
        assert_eq!(streak(&days, d("2026-11-08")).current, 0);
        assert_eq!(
            streak(&BTreeSet::new(), d("2026-11-06")),
            Streak {
                current: 0,
                best: 0
            }
        );
    }

    #[test]
    fn a_session_across_midnight_counts_both_days() {
        let start = Local
            .with_ymd_and_hms(2026, 11, 5, 23, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .with_ymd_and_hms(2026, 11, 6, 0, 40, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(session_days(start, end), [d("2026-11-05"), d("2026-11-06")]);
        assert_eq!(session_days(start, start), [d("2026-11-05")]);
    }

    fn row(name: &str, unlocked: Option<&str>) -> AchievementRow {
        AchievementRow {
            api_name: name.into(),
            name: Some(name.into()),
            description: None,
            icon_url: None,
            unlocked_at: unlocked.map(Into::into),
            global_percent: Some(12.0),
            hidden: false,
        }
    }

    #[test]
    fn only_unlocks_inside_the_session_window() {
        let rows = [
            row("before", Some("2026-11-06T19:00:00Z")),
            row("second", Some("2026-11-06T20:30:00Z")),
            row("first", Some("2026-11-06T19:20:00Z")),
            row("grace", Some("2026-11-06T21:37:30Z")),
            row("after", Some("2026-11-06T22:00:00Z")),
            row("locked", None),
        ];
        let names: Vec<_> = unlocked_during(&rows, at(19, 12), at(21, 36))
            .into_iter()
            .map(|r| r.api_name)
            .collect();
        assert_eq!(names, ["first", "second", "grace"]);
    }

    #[test]
    fn average_needs_three_other_sessions() {
        assert_eq!(average_seconds(&[600, 1200]), None);
        assert_eq!(average_seconds(&[600, 1200, 1800]), Some(1200));
    }
}
