//! Playtime cumul rule (ARCHITECTURE.md § 4 "Cumul"): never count the same
//! time twice.
//!
//! For each source of a game:
//! - if the platform provides its own counter (`source_playtime_minutes`),
//!   that counter is the source's time and its sessions are history only;
//! - otherwise the source's time is the sum of its sessions (any origin).
//!
//! Sessions not attached to a known source are added once, as "other".
//! The game's time is the sum over its sources.

use std::collections::HashMap;

/// Time of one source of a game, in minutes. `source_id = None` = sessions
/// without a source.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTime {
    pub source_id: Option<String>,
    pub minutes: i64,
    /// True when the minutes come from the platform's own counter.
    pub from_platform: bool,
}

/// `sources`: `(source_id, platform counter)`; `sessions`: `(source_id, seconds)`.
pub fn breakdown(
    sources: &[(String, Option<i64>)],
    sessions: &[(Option<String>, i64)],
) -> Vec<SourceTime> {
    let mut session_seconds: HashMap<Option<&str>, i64> = HashMap::new();
    for (source_id, seconds) in sessions {
        let key = source_id
            .as_deref()
            .filter(|id| sources.iter().any(|(s, _)| s == id));
        *session_seconds.entry(key).or_default() += seconds.max(&0);
    }

    let mut result: Vec<SourceTime> = sources
        .iter()
        .map(|(id, counter)| match counter {
            Some(minutes) => SourceTime {
                source_id: Some(id.clone()),
                minutes: (*minutes).max(0),
                from_platform: true,
            },
            None => SourceTime {
                source_id: Some(id.clone()),
                minutes: session_seconds
                    .get(&Some(id.as_str()))
                    .copied()
                    .unwrap_or(0)
                    / 60,
                from_platform: false,
            },
        })
        .collect();

    if let Some(&orphan) = session_seconds.get(&None) {
        if orphan > 0 {
            result.push(SourceTime {
                source_id: None,
                minutes: orphan / 60,
                from_platform: false,
            });
        }
    }
    result
}

pub fn total_minutes(parts: &[SourceTime]) -> i64 {
    parts.iter().map(|p| p.minutes).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(id: &str) -> Option<String> {
        Some(id.to_string())
    }

    #[test]
    fn platform_counter_wins_over_its_sessions() {
        let parts = breakdown(&[("steam".into(), Some(600))], &[(s("steam"), 3600 * 4)]);
        assert_eq!(total_minutes(&parts), 600);
        assert!(parts[0].from_platform);
    }

    #[test]
    fn sources_without_counter_sum_their_sessions() {
        let parts = breakdown(
            &[("gog".into(), Some(120)), ("local".into(), None)],
            &[(s("local"), 1800), (s("local"), 1830), (s("gog"), 99_999)],
        );
        assert_eq!(parts[0].minutes, 120);
        assert_eq!(parts[1].minutes, 60); // 3630 s → 60 min
        assert_eq!(total_minutes(&parts), 180);
    }

    #[test]
    fn orphan_sessions_are_counted_once() {
        let parts = breakdown(
            &[("steam".into(), Some(10))],
            &[(None, 600), (s("deleted-source"), 600)],
        );
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].source_id, None);
        assert_eq!(parts[1].minutes, 20);
        assert_eq!(total_minutes(&parts), 30);
    }

    #[test]
    fn nothing_played() {
        let parts = breakdown(&[("steam".into(), None)], &[]);
        assert_eq!(total_minutes(&parts), 0);
        assert!(breakdown(&[], &[]).is_empty());
    }

    #[test]
    fn negative_values_are_ignored() {
        let parts = breakdown(
            &[("a".into(), Some(-5)), ("b".into(), None)],
            &[(s("b"), -600)],
        );
        assert_eq!(total_minutes(&parts), 0);
    }
}
