//! End-of-session recap (docs/SESSION-RECAP.md): what the modal shows, what
//! it saves, and the events telling the frontend to open it.

use std::collections::{BTreeSet, HashSet};
use std::sync::Mutex;

use chrono::{DateTime, Local, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::CommandResult;
use crate::db::repo::goals::{self, Goal};
use crate::db::repo::sessions::{self, IdlePeriod, PlaySession, SessionOrigin};
use crate::db::repo::{achievements, artwork, game_sources, games};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::library::recap::{self, Streak, TimelineSegment};
use crate::library::{query, stats};
use crate::models::{GameStatus, SourceKind};

pub const RECAP_READY_EVENT: &str = "session-recap-ready";
pub const RECAP_UPDATED_EVENT: &str = "session-recap-updated";

/// Open goals listed in the modal.
const MAX_GOALS: usize = 5;
const MAX_RESUME_NOTE: usize = 2_000;

/// Sessions whose achievements are being resynced right now.
static SYNCING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn set_syncing(session_id: &str, on: bool) {
    let mut guard = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
    let set = guard.get_or_insert_with(HashSet::new);
    if on {
        set.insert(session_id.to_string());
    } else {
        set.remove(session_id);
    }
}

fn is_syncing(session_id: &str) -> bool {
    SYNCING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|s| s.contains(session_id))
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapEventPayload {
    pub session_id: String,
    pub game_id: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapGame {
    pub id: String,
    pub title: String,
    pub logo: Option<String>,
    pub hero: Option<String>,
    pub header: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapAchievement {
    pub name: String,
    pub icon: Option<String>,
    pub global_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapAchievements {
    pub unlocked: Vec<RecapAchievement>,
    pub done: u32,
    pub total: u32,
    /// Resync after the session still running: show a skeleton.
    pub syncing: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapWeek {
    pub minutes: u32,
    pub sessions: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecap {
    pub session: PlaySession,
    pub game: RecapGame,
    pub active_seconds: u32,
    pub idle_periods: Vec<IdlePeriod>,
    pub timeline: Vec<TimelineSegment>,
    /// Average of the other `auto` / `manual` sessions (3 at least).
    pub average_seconds: Option<u32>,
    pub total_minutes: u32,
    /// Platform counter (Steam) not resynced yet: the total is an estimate.
    pub total_approx: bool,
    pub week: RecapWeek,
    pub streak: Streak,
    /// `None`: no source of the game has achievements.
    pub achievements: Option<RecapAchievements>,
    pub open_goals: Vec<Goal>,
    pub open_goals_total: u32,
    pub resume_note: Option<String>,
    pub rating: Option<u8>,
    pub status: GameStatus,
}

fn parse(t: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(t)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| AppError::Invalid(format!("bad date {t}")))
}

/// Source whose achievements the game shows: emulator, then Steam, then the
/// first one with a cache (GOG). Same order as `achievements_get`.
fn achievement_source(sources: &[game_sources::GameSource]) -> Option<&game_sources::GameSource> {
    sources
        .iter()
        .find(|s| s.kind == SourceKind::Emulator)
        .or_else(|| sources.iter().find(|s| s.kind == SourceKind::Steam))
        .or_else(|| sources.iter().find(|s| s.kind == SourceKind::Gog))
}

fn played_days(conn: &Connection) -> AppResult<BTreeSet<chrono::NaiveDate>> {
    let mut stmt = conn.prepare(
        "SELECT started_at, ended_at FROM play_sessions \
         WHERE origin IN ('auto', 'manual') AND started_at IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
    })?;
    let mut days = BTreeSet::new();
    for row in rows {
        let (start, end) = row?;
        let Ok(start) = parse(&start) else { continue };
        let end = end.and_then(|e| parse(&e).ok()).unwrap_or(start);
        days.extend(recap::session_days(start, end));
    }
    Ok(days)
}

/// `icons`: the achievement icon cache folder (icons are never hotlinked).
pub fn build(
    conn: &Connection,
    session_id: &str,
    syncing: bool,
    icons: Option<&std::path::Path>,
) -> AppResult<SessionRecap> {
    let session = sessions::get(conn, session_id)?;
    let game = games::get(conn, &session.game_id)?;
    let start = parse(session.started_at.as_deref().unwrap_or_default())?;
    let end = session
        .ended_at
        .as_deref()
        .map(parse)
        .transpose()?
        .unwrap_or(start);
    let idle_periods = sessions::idle_periods(conn, session_id)?;
    let idle: Vec<_> = idle_periods
        .iter()
        .filter_map(|p| Some((parse(&p.started_at).ok()?, parse(&p.ended_at).ok()?)))
        .collect();

    let others: Vec<u32> = sessions::list_for_game(conn, &game.id)?
        .into_iter()
        .filter(|s| {
            s.id != session.id && matches!(s.origin, SessionOrigin::Auto | SessionOrigin::Manual)
        })
        .map(|s| s.duration_seconds)
        .collect();

    let summary = query::summaries(conn)?
        .into_iter()
        .find(|g| g.id == game.id)
        .ok_or_else(|| AppError::NotFound(format!("game {}", game.id)))?;
    let sources = game_sources::list_for_game(conn, &game.id)?;
    // Steam counts its own time: this session is not in the total until the
    // counter is read again, so it is added here as an estimate.
    let counter = session
        .game_source_id
        .as_ref()
        .and_then(|id| sources.iter().find(|s| &s.id == id))
        .is_some_and(|s| s.source_playtime_minutes.is_some());
    let total_minutes = if counter {
        summary.playtime_minutes + session.duration_seconds / 60
    } else {
        summary.playtime_minutes
    };

    let today = Local::now().date_naive();
    let week = stats::overview(conn, stats::StatsRange::Week, &[], today)?;

    let achievements = match achievement_source(&sources) {
        None => None,
        Some(src) => {
            let rows = achievements::list_for_source(conn, &src.id)?;
            if rows.is_empty() && !syncing {
                None
            } else {
                Some(RecapAchievements {
                    unlocked: recap::unlocked_during(&rows, start, end)
                        .into_iter()
                        .map(|r| RecapAchievement {
                            name: r.name.unwrap_or(r.api_name),
                            icon: r
                                .icon_url
                                .as_deref()
                                .zip(icons)
                                .and_then(|(url, dir)| {
                                    crate::metadata::achievement_icons::cached(dir, url)
                                })
                                .map(|p| p.display().to_string()),
                            global_percent: r.global_percent,
                        })
                        .collect(),
                    done: rows.iter().filter(|r| r.unlocked_at.is_some()).count() as u32,
                    total: rows.len() as u32,
                    syncing,
                })
            }
        }
    };

    let open: Vec<Goal> = goals::list(conn, &game.id)?
        .into_iter()
        .filter(|g| g.done_at.is_none())
        .collect();
    let mut art = artwork::for_game(conn, &game.id)?;

    Ok(SessionRecap {
        active_seconds: session.duration_seconds,
        timeline: recap::timeline(start, end, &idle),
        idle_periods,
        average_seconds: recap::average_seconds(&others),
        total_minutes,
        total_approx: counter,
        week: RecapWeek {
            minutes: week.total_minutes,
            sessions: week.sessions_in_range,
        },
        streak: recap::streak(&played_days(conn)?, today),
        achievements,
        open_goals_total: open.len() as u32,
        open_goals: open.into_iter().take(MAX_GOALS).collect(),
        resume_note: games::resume_note(conn, &game.id)?.map(|(n, _)| n),
        rating: sessions::rating(conn, session_id)?,
        status: game.status,
        game: RecapGame {
            id: game.id,
            title: game.title,
            logo: art.remove("logo"),
            hero: art.remove("hero"),
            header: summary.header,
        },
        session,
    })
}

#[tauri::command]
#[specta::specta]
pub fn session_recap(
    db: tauri::State<'_, Db>,
    paths: tauri::State<'_, super::app::AppPaths>,
    session_id: String,
) -> CommandResult<SessionRecap> {
    let icons = crate::metadata::achievement_icons::dir(&paths.data_dir);
    Ok(build(
        &db.conn(),
        &session_id,
        is_syncing(&session_id),
        Some(&icons),
    )?)
}

#[derive(Debug, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecapSave {
    /// "Où t'es-tu arrêté ?"; blank clears it.
    pub resume_note: Option<String>,
    /// 1 to 5 flames; 0 clears the rating.
    pub rating: Option<u8>,
    pub status: Option<GameStatus>,
}

pub fn save(conn: &Connection, session_id: &str, input: &RecapSave) -> AppResult<()> {
    let session = sessions::get(conn, session_id)?;
    if let Some(note) = &input.resume_note {
        if note.chars().count() > MAX_RESUME_NOTE {
            return Err(AppError::Invalid("note is too long".into()));
        }
        games::set_resume_note(conn, &session.game_id, Some(note))?;
        // Kept on the session too, for the history in the session log.
        let trimmed = note.trim();
        sessions::set_note(conn, session_id, (!trimmed.is_empty()).then_some(trimmed))?;
    }
    if let Some(rating) = input.rating {
        sessions::set_rating(conn, session_id, (rating > 0).then_some(rating))?;
    }
    if let Some(status) = input.status {
        games::update(
            conn,
            &session.game_id,
            &games::GamePatch {
                status: Some(status),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn session_recap_save(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    session_id: String,
    input: RecapSave,
) -> CommandResult<()> {
    save(&db.conn(), &session_id, &input)?;
    let _ = app.emit(super::sources::LIBRARY_CHANGED_EVENT, ());
    Ok(())
}

/// "Ne pas compter cette session": the session and its time are removed.
#[tauri::command]
#[specta::specta]
pub fn session_discard(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    session_id: String,
) -> CommandResult<()> {
    sessions::delete(&db.conn(), &session_id)?;
    let _ = app.emit(super::sources::LIBRARY_CHANGED_EVENT, ());
    Ok(())
}

/// Brings Ignite's window to the front (out of the notification area).
#[tauri::command]
#[specta::specta]
pub fn window_reveal(app: AppHandle) {
    crate::tray::show_main(&app);
}

/// Windows notification "Session terminée · jeu · 2 h 14" (texts from the UI),
/// used when the modal cannot open now (couch mode, another game running).
#[tauri::command]
#[specta::specta]
pub fn notify(app: AppHandle, title: String, body: String) -> CommandResult<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| AppError::Invalid(format!("notification: {e}")).into())
}

/// Called by the tracker once an automatic session is written: the modal can
/// open at once, achievements are resynced in the background and the recap
/// is refreshed when they are in.
pub fn session_written(app: &AppHandle, session_id: &str, game_id: &str) {
    let payload = RecapEventPayload {
        session_id: session_id.to_string(),
        game_id: game_id.to_string(),
    };
    let has_achievements = {
        let db = app.state::<Db>();
        let conn = db.conn();
        game_sources::list_for_game(&conn, game_id)
            .map(|s| achievement_source(&s).is_some())
            .unwrap_or(false)
    };
    if has_achievements {
        set_syncing(session_id, true);
    }
    let _ = app.emit(RECAP_READY_EVENT, payload.clone());
    if !has_achievements {
        return;
    }
    let app = app.clone();
    let session_id = session_id.to_string();
    let game_id = game_id.to_string();
    std::thread::spawn(move || {
        if let Err(e) = super::achievements::refresh(&app, &game_id) {
            tracing::warn!(error = ?e, "recap: achievements resync failed");
        }
        super::achievements::fetch_icons_now(&app, &game_id);
        set_syncing(&session_id, false);
        let _ = app.emit(RECAP_UPDATED_EVENT, payload);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto(id: &str, game: &str, start: &str, end: &str, secs: u32) -> PlaySession {
        PlaySession {
            id: id.into(),
            game_id: game.into(),
            game_source_id: None,
            started_at: Some(start.into()),
            ended_at: Some(end.into()),
            duration_seconds: secs,
            idle_seconds: 0,
            origin: SessionOrigin::Auto,
            note: None,
            rating: None,
        }
    }

    #[test]
    fn recap_then_save_then_discard() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        for (i, secs) in [1800, 2400, 3000].iter().enumerate() {
            let day = format!("2026-10-0{}T18:00:00Z", i + 1);
            sessions::save(&conn, &auto(&format!("old{i}"), &g.id, &day, &day, *secs)).unwrap();
        }
        let s = auto(
            "now",
            &g.id,
            "2026-11-06T18:00:00Z",
            "2026-11-06T20:24:00Z",
            8040,
        );
        sessions::save(&conn, &s).unwrap();
        sessions::replace_idle_periods(
            &conn,
            "now",
            &[IdlePeriod {
                started_at: "2026-11-06T19:00:00Z".into(),
                ended_at: "2026-11-06T19:06:00Z".into(),
            }],
        )
        .unwrap();
        goals::add(&conn, &g.id, "Finir l'acte 3").unwrap();

        let r = build(&conn, "now", false, None).unwrap();
        assert_eq!(r.average_seconds, Some(2400));
        assert_eq!(r.total_minutes, (1800 + 2400 + 3000 + 8040) / 60);
        assert!(!r.total_approx);
        assert_eq!(r.timeline.len(), 3);
        assert!(r.timeline[1].idle);
        assert!(r.achievements.is_none());
        assert_eq!(r.open_goals.len(), 1);
        assert_eq!(r.status, GameStatus::Todo);

        save(
            &conn,
            "now",
            &RecapSave {
                resume_note: Some("  Acte 3, avant le boss  ".into()),
                rating: Some(4),
                status: Some(GameStatus::Playing),
            },
        )
        .unwrap();
        let r = build(&conn, "now", false, None).unwrap();
        assert_eq!(r.resume_note.as_deref(), Some("Acte 3, avant le boss"));
        assert_eq!(r.session.note.as_deref(), Some("Acte 3, avant le boss"));
        assert_eq!(r.rating, Some(4));
        assert_eq!(r.status, GameStatus::Playing);
        assert!(save(
            &conn,
            "now",
            &RecapSave {
                rating: Some(6),
                ..Default::default()
            }
        )
        .is_err());

        // "Ne pas compter cette session": its time leaves the total.
        sessions::delete(&conn, "now").unwrap();
        let total = query::summaries(&conn)
            .unwrap()
            .into_iter()
            .find(|x| x.id == g.id)
            .unwrap()
            .playtime_minutes;
        assert_eq!(total, (1800 + 2400 + 3000) / 60);
        assert!(sessions::idle_periods(&conn, "now").unwrap().is_empty());
    }
}
