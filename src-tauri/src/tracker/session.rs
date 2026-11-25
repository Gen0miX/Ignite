//! Session state machine (ARCHITECTURE.md § 4), free of I/O so it can be
//! tested tick by tick. The tracker service feeds it every few seconds with
//! the running games and the system idle time, and persists its events.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackerConfig {
    /// Sessions shorter than this are discarded (crash, launched by mistake).
    pub min_session_secs: i64,
    /// No keyboard / mouse / gamepad input for this long = idle.
    pub idle_threshold_secs: i64,
    /// A game is over once its processes are gone for this long, during the
    /// first `startup_secs` (launchers often hand over to a child process).
    pub grace_secs: i64,
    /// Past the start-up phase, a closed game is over after this short delay,
    /// so the recap shows right after quitting.
    pub settled_grace_secs: i64,
    pub startup_secs: i64,
    /// How often an ongoing session is written, to survive a crash.
    pub heartbeat_secs: i64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            min_session_secs: 120,
            idle_threshold_secs: 15 * 60,
            grace_secs: 10,
            settled_grace_secs: 4,
            startup_secs: 180,
            heartbeat_secs: 60,
        }
    }
}

/// An ongoing session.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSession {
    pub session_id: String,
    pub game_id: String,
    pub source_id: Option<String>,
    pub started_at: DateTime<Utc>,
    /// Last tick where a matching process was running.
    pub last_seen: DateTime<Utc>,
    pub idle_seconds: i64,
    /// Idle stretches removed from the session (`start`, `end`), for the
    /// recap timeline; the last one grows while the stretch lasts.
    pub idle_periods: Vec<(DateTime<Utc>, DateTime<Utc>)>,
    last_heartbeat: DateTime<Utc>,
    /// The current idle stretch has already been counted.
    in_idle_stretch: bool,
}

impl ActiveSession {
    pub fn total_seconds(&self) -> i64 {
        (self.last_seen - self.started_at).num_seconds().max(0)
    }

    /// Active play time: total minus idle.
    pub fn active_seconds(&self) -> i64 {
        (self.total_seconds() - self.idle_seconds).max(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrackerEvent {
    /// Not written yet: the row appears with the first heartbeat.
    Started(ActiveSession),
    /// Ongoing session to write; only once it reached the minimum.
    Heartbeat(ActiveSession),
    /// `keep = false` when the session is shorter than the minimum: delete it.
    Ended { session: ActiveSession, keep: bool },
}

pub struct SessionTracker {
    config: TrackerConfig,
    active: BTreeMap<String, ActiveSession>,
    last_tick: Option<DateTime<Utc>>,
    new_id: Box<dyn FnMut() -> String + Send>,
}

impl SessionTracker {
    pub fn new(config: TrackerConfig) -> Self {
        Self::with_id_generator(config, Box::new(crate::db::new_id))
    }

    pub fn with_id_generator(
        config: TrackerConfig,
        new_id: Box<dyn FnMut() -> String + Send>,
    ) -> Self {
        Self {
            config,
            active: BTreeMap::new(),
            last_tick: None,
            new_id,
        }
    }

    pub fn set_config(&mut self, config: TrackerConfig) {
        self.config = config;
    }

    pub fn active(&self) -> impl Iterator<Item = &ActiveSession> {
        self.active.values()
    }

    /// `running`: `(game_id, source_id)` with a live process.
    /// `idle_for_secs`: seconds since the last user input, system-wide.
    pub fn tick(
        &mut self,
        now: DateTime<Utc>,
        running: &[(String, String)],
        idle_for_secs: i64,
    ) -> Vec<TrackerEvent> {
        let mut events = Vec::new();
        let dt = self
            .last_tick
            .map(|t| (now - t).num_seconds().max(0))
            .unwrap_or(0);
        self.last_tick = Some(now);
        let idle = idle_for_secs >= self.config.idle_threshold_secs;

        for (game_id, source_id) in running {
            match self.active.get_mut(game_id) {
                Some(s) => {
                    s.last_seen = now;
                    if idle {
                        if s.in_idle_stretch {
                            s.idle_seconds += dt;
                            if let Some(last) = s.idle_periods.last_mut() {
                                last.1 = now;
                            }
                        } else {
                            // The whole stretch was idle, not only what exceeds the threshold.
                            let back = idle_for_secs.min(s.total_seconds());
                            s.idle_seconds += back;
                            s.in_idle_stretch = true;
                            s.idle_periods
                                .push((now - chrono::Duration::seconds(back), now));
                        }
                        s.idle_seconds = s.idle_seconds.min(s.total_seconds());
                    } else {
                        s.in_idle_stretch = false;
                    }
                    if (now - s.last_heartbeat).num_seconds() >= self.config.heartbeat_secs {
                        s.last_heartbeat = now;
                        // Nothing is written before the minimum: a game closed
                        // (or a crash) within it leaves no row behind.
                        if s.active_seconds() >= self.config.min_session_secs {
                            events.push(TrackerEvent::Heartbeat(s.clone()));
                        }
                    }
                }
                None => {
                    let s = ActiveSession {
                        session_id: (self.new_id)(),
                        game_id: game_id.clone(),
                        source_id: Some(source_id.clone()),
                        started_at: now,
                        last_seen: now,
                        idle_seconds: 0,
                        idle_periods: Vec::new(),
                        last_heartbeat: now,
                        in_idle_stretch: idle,
                    };
                    events.push(TrackerEvent::Started(s.clone()));
                    self.active.insert(game_id.clone(), s);
                }
            }
        }

        let config = self.config;
        let gone: Vec<String> = self
            .active
            .iter()
            .filter(|(id, s)| {
                let grace = if (s.last_seen - s.started_at).num_seconds() >= config.startup_secs {
                    config.settled_grace_secs
                } else {
                    config.grace_secs
                };
                !running.iter().any(|(g, _)| g == *id) && (now - s.last_seen).num_seconds() > grace
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            if let Some(session) = self.active.remove(&id) {
                let keep = session.active_seconds() >= self.config.min_session_secs;
                events.push(TrackerEvent::Ended { session, keep });
            }
        }
        events
    }

    /// Ends every session (app shutting down). Sessions keep their last state.
    pub fn stop_all(&mut self) -> Vec<TrackerEvent> {
        std::mem::take(&mut self.active)
            .into_values()
            .map(|session| {
                let keep = session.active_seconds() >= self.config.min_session_secs;
                TrackerEvent::Ended { session, keep }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + secs, 0).unwrap()
    }

    fn tracker() -> SessionTracker {
        let mut n = 0;
        SessionTracker::with_id_generator(
            TrackerConfig::default(),
            Box::new(move || {
                n += 1;
                format!("s{n}")
            }),
        )
    }

    fn hades() -> Vec<(String, String)> {
        vec![("hades".into(), "src".into())]
    }

    /// Runs the game from `from` to `to` (exclusive) with 3 s ticks.
    fn play(
        tr: &mut SessionTracker,
        from: i64,
        to: i64,
        idle: impl Fn(i64) -> i64,
    ) -> Vec<TrackerEvent> {
        let mut events = Vec::new();
        let mut s = from;
        while s < to {
            events.extend(tr.tick(t(s), &hades(), idle(s)));
            s += 3;
        }
        events
    }

    fn ended(events: &[TrackerEvent]) -> Option<(&ActiveSession, bool)> {
        events.iter().find_map(|e| match e {
            TrackerEvent::Ended { session, keep } => Some((session, *keep)),
            _ => None,
        })
    }

    #[test]
    fn a_normal_session_starts_beats_and_ends_after_the_grace_period() {
        let mut tr = tracker();
        let events = play(&mut tr, 0, 600, |_| 0);
        assert!(matches!(&events[0], TrackerEvent::Started(s) if s.session_id == "s1"));
        let beats = events
            .iter()
            .filter(|e| matches!(e, TrackerEvent::Heartbeat(_)))
            .count();
        assert_eq!(beats, 8); // every 60 s over ~10 min, from the 2 min minimum on

        // Process gone: still active for a few seconds...
        assert!(tr.tick(t(600), &[], 0).is_empty());
        // ...ended right after (settled session), with the last time it was seen as end.
        let events = tr.tick(t(603), &[], 0);
        let (s, keep) = ended(&events).unwrap();
        assert!(keep);
        assert_eq!(s.last_seen, t(597));
        assert_eq!(s.active_seconds(), 597);
        assert_eq!(tr.active().count(), 0);
    }

    #[test]
    fn idle_stretches_are_recorded_for_the_timeline() {
        let mut tr = tracker();
        // Inputs stop at 300 s; idle (15 min threshold) is noticed at 1200 s,
        // inputs come back at 1500 s.
        let events = play(&mut tr, 0, 2000, |s| {
            if (300..1500).contains(&s) {
                s - 300
            } else {
                0
            }
        });
        let _ = events;
        let s = tr.active().next().unwrap();
        assert_eq!(s.idle_periods.len(), 1);
        let (start, end) = s.idle_periods[0];
        assert_eq!(start, t(300));
        assert_eq!(end, t(1497));
        assert_eq!(s.idle_seconds, (end - start).num_seconds());
    }

    #[test]
    fn a_session_under_the_minimum_never_asks_to_be_written() {
        let mut tr = tracker();
        let mut events = play(&mut tr, 0, 30, |_| 0);
        events.extend(tr.tick(t(45), &[], 0));
        assert!(!events
            .iter()
            .any(|e| matches!(e, TrackerEvent::Heartbeat(_))));
        let (_, keep) = ended(&events).unwrap();
        assert!(!keep);
    }

    #[test]
    fn a_process_restart_within_the_grace_period_keeps_one_session() {
        let mut tr = tracker();
        play(&mut tr, 0, 60, |_| 0);
        // Launcher hands over to the game: 9 s without a process.
        assert!(tr.tick(t(63), &[], 0).is_empty());
        assert!(tr.tick(t(66), &[], 0).is_empty());
        let events = play(&mut tr, 66, 400, |_| 0);
        assert!(events
            .iter()
            .all(|e| !matches!(e, TrackerEvent::Started(_))));
        assert_eq!(tr.active().next().unwrap().session_id, "s1");
    }

    #[test]
    fn a_closed_game_ends_within_seconds_once_settled() {
        let mut tr = tracker();
        // Polled every second while a game runs.
        for s in 0..=600 {
            tr.tick(t(s), &hades(), 0);
        }
        assert!(tr.tick(t(604), &[], 0).is_empty());
        let events = tr.tick(t(605), &[], 0);
        let (s, keep) = ended(&events).unwrap();
        assert!(keep);
        assert_eq!(s.last_seen, t(600));
    }

    #[test]
    fn short_sessions_are_discarded() {
        let mut tr = tracker();
        play(&mut tr, 0, 60, |_| 0);
        let events = tr.tick(t(100), &[], 0);
        let (_, keep) = ended(&events).unwrap();
        assert!(!keep);
    }

    #[test]
    fn idle_stretches_are_removed_entirely() {
        let mut tr = tracker();
        // 10 min of play, then 20 min without input, then 5 min of play.
        let events = play(&mut tr, 0, 35 * 60, |s| {
            if (600..1800).contains(&s) {
                s - 600
            } else {
                0
            }
        });
        assert!(ended(&events).is_none());
        let s = tr.active().next().unwrap().clone();
        let idle = s.idle_seconds;
        // The 20-minute stretch is removed (± one tick).
        assert!((1197..=1203).contains(&idle), "idle = {idle}");
        assert!((897..=903).contains(&s.active_seconds()));
    }

    #[test]
    fn idle_never_exceeds_the_session_length() {
        let mut tr = tracker();
        // The game starts while the user is already away for an hour.
        play(&mut tr, 0, 120, |_| 3600);
        let s = tr.active().next().unwrap();
        assert!(s.idle_seconds <= s.total_seconds());
        assert_eq!(s.active_seconds(), 0);
    }

    #[test]
    fn two_games_are_tracked_independently() {
        let mut tr = tracker();
        let both = vec![
            ("a".to_string(), "sa".to_string()),
            ("b".to_string(), "sb".to_string()),
        ];
        let e = tr.tick(t(0), &both, 0);
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, TrackerEvent::Started(_)))
                .count(),
            2
        );
        tr.tick(t(200), &both, 0);
        // b stops: still counted at 203, ended at 205 (settled session).
        assert!(ended(&tr.tick(t(203), &both[..1], 0)).is_none());
        let e = tr.tick(t(205), &both[..1], 0);
        let (s, _) = ended(&e).unwrap();
        assert_eq!(s.game_id, "b");
        assert_eq!(tr.active().count(), 1);
    }

    #[test]
    fn stop_all_ends_everything() {
        let mut tr = tracker();
        play(&mut tr, 0, 300, |_| 0);
        let events = tr.stop_all();
        assert_eq!(events.len(), 1);
        assert_eq!(tr.active().count(), 0);
    }
}
