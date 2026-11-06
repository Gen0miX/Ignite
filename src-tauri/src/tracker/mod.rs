//! Playtime tracking (ARCHITECTURE.md § 4): a background thread polls the
//! running processes every 3 s, feeds the session state machine and writes
//! sessions to SQLite (heartbeat every 60 s, so a crash loses at most a minute).

pub mod matcher;
pub mod session;
pub mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::repo::sessions::{self, PlaySession, SessionOrigin};
use crate::db::repo::{game_sources, settings};
use crate::db::Db;
use matcher::WatchEntry;
use session::{ActiveSession, SessionTracker, TrackerConfig, TrackerEvent};

pub const SESSION_STARTED_EVENT: &str = "session-started";
pub const SESSION_ENDED_EVENT: &str = "session-ended";

const POLL: Duration = Duration::from_secs(3);
const RELOAD: Duration = Duration::from_secs(30);

pub const MIN_SESSION_KEY: &str = "tracking.min_session_minutes";
pub const IDLE_KEY: &str = "tracking.idle_minutes";

/// A game being played right now (for the "En jeu" badge and live timer).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveGame {
    pub session_id: String,
    pub game_id: String,
    pub started_at: String,
    /// Active seconds at the last poll.
    pub active_seconds: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionEventPayload {
    game_id: String,
    session_id: String,
}

/// Shared tracker state in Tauri.
#[derive(Default)]
pub struct TrackerState {
    active: Mutex<Vec<ActiveGame>>,
    /// Game last launched from Ludothèque (resolves emulator ties), 2 h.
    hint: Mutex<Option<(String, Instant)>>,
    stop: AtomicBool,
    stopped: Arc<(Mutex<bool>, std::sync::Condvar)>,
}

impl TrackerState {
    pub fn set_launch_hint(&self, game_id: &str) {
        *self.hint.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((game_id.to_string(), Instant::now()));
    }

    fn launch_hint(&self) -> Option<String> {
        self.hint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(2 * 3600))
            .map(|(id, _)| id.clone())
    }

    pub fn active(&self) -> Vec<ActiveGame> {
        self.active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Asks the loop to close open sessions and waits a little for it.
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let (lock, cvar) = &*self.stopped;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        let _ = cvar.wait_timeout_while(guard, Duration::from_secs(2), |done| !*done);
    }
}

fn rfc3339(t: chrono::DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn to_row(s: &ActiveSession) -> PlaySession {
    PlaySession {
        id: s.session_id.clone(),
        game_id: s.game_id.clone(),
        game_source_id: s.source_id.clone(),
        started_at: Some(rfc3339(s.started_at)),
        ended_at: Some(rfc3339(s.last_seen)),
        duration_seconds: s.active_seconds().clamp(0, u32::MAX as i64) as u32,
        idle_seconds: s.idle_seconds.clamp(0, u32::MAX as i64) as u32,
        origin: SessionOrigin::Auto,
        note: None,
    }
}

/// Tracking settings, with the SPEC defaults (2 min minimum, 15 min idle).
pub fn load_config(db: &Db) -> TrackerConfig {
    let conn = db.conn();
    let minutes = |key, default: i64| -> i64 {
        settings::get::<i64>(&conn, key)
            .ok()
            .flatten()
            .filter(|m| *m >= 0)
            .unwrap_or(default)
    };
    TrackerConfig {
        min_session_secs: minutes(MIN_SESSION_KEY, 2) * 60,
        idle_threshold_secs: minutes(IDLE_KEY, 15).max(1) * 60,
        ..TrackerConfig::default()
    }
}

/// Installed sources with at least one process matcher.
fn load_watch_list(db: &Db) -> Vec<WatchEntry> {
    let sources = match game_sources::list_all(&db.conn()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "tracker: cannot load sources");
            return Vec::new();
        }
    };
    sources
        .into_iter()
        .filter(|s| s.installed)
        .filter_map(|s| {
            let matchers = matcher::matchers_for(
                s.process_matchers.as_deref(),
                s.install_path.as_deref(),
                s.exe_path.as_deref(),
            );
            (!matchers.is_empty()).then_some(WatchEntry {
                game_id: s.game_id,
                source_id: s.id,
                matchers,
            })
        })
        .collect()
}

/// Writes one tracker event: nothing on start (the row appears with the first
/// heartbeat, once the minimum is reached), the row on heartbeat and on a kept
/// end, a delete on a discarded end (in case a heartbeat wrote it).
fn write_event(conn: &rusqlite::Connection, event: &TrackerEvent) -> crate::error::AppResult<()> {
    match event {
        TrackerEvent::Started(_) => Ok(()),
        TrackerEvent::Heartbeat(s) => sessions::save(conn, &to_row(s)),
        TrackerEvent::Ended { session: s, keep } => {
            if *keep {
                sessions::save(conn, &to_row(s))
            } else {
                sessions::delete(conn, &s.session_id)
            }
        }
    }
}

fn persist(app: &AppHandle, events: Vec<TrackerEvent>) {
    let db = app.state::<Db>();
    for event in events {
        if let Err(e) = write_event(&db.conn(), &event) {
            tracing::warn!(error = %e, "tracker: cannot write session");
        }
        match &event {
            TrackerEvent::Started(s) => {
                tracing::info!(game = %s.game_id, "session started");
                let _ = app.emit(
                    SESSION_STARTED_EVENT,
                    SessionEventPayload {
                        game_id: s.game_id.clone(),
                        session_id: s.session_id.clone(),
                    },
                );
            }
            TrackerEvent::Heartbeat(_) => {}
            TrackerEvent::Ended { session: s, keep } => {
                tracing::info!(game = %s.game_id, active = s.active_seconds(), keep, "session ended");
                let _ = app.emit(
                    SESSION_ENDED_EVENT,
                    SessionEventPayload {
                        game_id: s.game_id.clone(),
                        session_id: s.session_id.clone(),
                    },
                );
                let _ = app.emit(crate::commands::sources::LIBRARY_CHANGED_EVENT, ());
            }
        }
    }
}

/// Removes automatic sessions under the minimum left by older versions or by
/// an app killed mid-session (installer update, crash).
fn purge_short_sessions(db: &Db, config: &TrackerConfig) {
    match sessions::purge_short_auto(&db.conn(), config.min_session_secs) {
        Ok(0) => {}
        Ok(n) => tracing::info!(removed = n, "tracker: short sessions purged"),
        Err(e) => tracing::warn!(error = %e, "tracker: cannot purge short sessions"),
    }
}

/// Starts the tracking loop on its own thread.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("tracker".into())
        .spawn(move || run(app))
        .expect("failed to start the tracker thread");
}

fn run(app: AppHandle) {
    let state = app.state::<TrackerState>();
    let mut processes = system::ProcessProbe::new();
    let mut idle = system::IdleProbe::new();
    let config = load_config(&app.state::<Db>());
    purge_short_sessions(&app.state::<Db>(), &config);
    let mut tracker = SessionTracker::new(config);
    let mut watch = load_watch_list(&app.state::<Db>());
    let mut last_reload = Instant::now();
    tracing::info!(watched = watch.len(), "tracker started");

    while !state.stop.load(Ordering::SeqCst) {
        if last_reload.elapsed() >= RELOAD {
            let db = app.state::<Db>();
            watch = load_watch_list(&db);
            tracker.set_config(load_config(&db));
            last_reload = Instant::now();
        }

        let hint = state.launch_hint();
        let running = matcher::running_games(&watch, &processes.processes(), hint.as_deref());
        let events = tracker.tick(Utc::now(), &running, idle.idle_seconds());
        if !events.is_empty() {
            persist(&app, events);
        }
        *state.active.lock().unwrap_or_else(|e| e.into_inner()) = tracker
            .active()
            .map(|s| ActiveGame {
                session_id: s.session_id.clone(),
                game_id: s.game_id.clone(),
                started_at: rfc3339(s.started_at),
                active_seconds: s.active_seconds().clamp(0, u32::MAX as i64) as u32,
            })
            .collect();

        std::thread::sleep(POLL);
    }

    // Closing the app: write what is open.
    persist(&app, tracker.stop_all());
    let (lock, cvar) = &*state.stopped;
    *lock.lock().unwrap_or_else(|e| e.into_inner()) = true;
    cvar.notify_all();
    tracing::info!("tracker stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games;
    use chrono::TimeZone;

    /// Plays a game for `secs` (3 s ticks), closes it and writes every event.
    fn play_for(secs: i64) -> (Db, String) {
        let db = Db::open_in_memory().unwrap();
        let game = games::insert(&db.conn(), "Elden Ring").unwrap();
        let mut tr = SessionTracker::new(TrackerConfig::default());
        let at = |s: i64| Utc.timestamp_opt(1_800_000_000 + s, 0).unwrap();
        let running = vec![(game.id.clone(), "src".to_string())];
        let mut events = Vec::new();
        let mut s = 0;
        while s < secs {
            events.extend(tr.tick(at(s), &running, 0));
            s += 3;
        }
        events.extend(tr.tick(at(secs + 30), &[], 0));
        for mut e in events {
            // No game source row in this test.
            match &mut e {
                TrackerEvent::Started(a) | TrackerEvent::Heartbeat(a) => a.source_id = None,
                TrackerEvent::Ended { session, .. } => session.source_id = None,
            }
            write_event(&db.conn(), &e).unwrap();
        }
        (db, game.id)
    }

    #[test]
    fn a_game_closed_after_30_seconds_leaves_no_trace() {
        let (db, game) = play_for(30);
        assert!(sessions::list_for_game(&db.conn(), &game)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_ten_minute_game_is_written_once() {
        let (db, game) = play_for(600);
        let rows = sessions::list_for_game(&db.conn(), &game).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].duration_seconds >= 590);
    }

    #[test]
    fn short_auto_sessions_are_purged_but_not_manual_ones() {
        let db = Db::open_in_memory().unwrap();
        let game = games::insert(&db.conn(), "Elden Ring").unwrap();
        let row = |id: &str, secs: u32, origin: SessionOrigin| PlaySession {
            id: id.into(),
            game_id: game.id.clone(),
            game_source_id: None,
            started_at: Some("2026-11-06T10:00:00Z".into()),
            ended_at: Some("2026-11-06T10:01:00Z".into()),
            duration_seconds: secs,
            idle_seconds: 0,
            origin,
            note: None,
        };
        for s in [
            row("a", 20, SessionOrigin::Auto),
            row("b", 600, SessionOrigin::Auto),
            row("c", 30, SessionOrigin::Manual),
        ] {
            sessions::save(&db.conn(), &s).unwrap();
        }
        purge_short_sessions(&db, &TrackerConfig::default());
        let mut ids: Vec<_> = sessions::list_for_game(&db.conn(), &game.id)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["b", "c"]);
    }
}
