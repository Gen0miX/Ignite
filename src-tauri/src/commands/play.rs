//! Sessions, personal goals and live tracking.

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Deserialize;

use super::CommandResult;
use crate::db::repo::goals::{self, Goal};
use crate::db::repo::sessions::{self, PlaySession, SessionOrigin};
use crate::db::repo::{game_sources, games};
use crate::db::{new_id, Db};
use crate::error::{AppError, AppResult};
use crate::tracker::{ActiveGame, TrackerState};

#[tauri::command]
#[specta::specta]
pub fn tracker_active(state: tauri::State<'_, TrackerState>) -> Vec<ActiveGame> {
    state.active()
}

#[tauri::command]
#[specta::specta]
pub fn sessions_list(db: tauri::State<'_, Db>, game_id: String) -> CommandResult<Vec<PlaySession>> {
    Ok(sessions::list_for_game(&db.conn(), &game_id)?)
}

/// A session typed by the user. `id = None` creates one.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {
    pub id: Option<String>,
    pub game_id: String,
    /// RFC 3339; required for a manual session, ignored for an estimate.
    pub started_at: Option<String>,
    pub duration_minutes: u32,
    /// `manual` or `estimate` when creating; kept as is when editing.
    pub origin: SessionOrigin,
    pub note: Option<String>,
}

/// One manual session at most this long (longer: use an estimate).
const MAX_MANUAL_MINUTES: u32 = 24 * 60;
/// "Already played" estimates: up to 100 000 h.
const MAX_ESTIMATE_MINUTES: u32 = 6_000_000;

/// Validates user input and builds the row to save. `existing` is the row
/// being edited, if any.
pub fn build_session(
    input: &SessionInput,
    existing: Option<&PlaySession>,
    now: DateTime<Utc>,
) -> AppResult<PlaySession> {
    let origin = existing.map(|s| s.origin).unwrap_or(input.origin);
    if existing.is_none() && !matches!(origin, SessionOrigin::Manual | SessionOrigin::Estimate) {
        return Err(AppError::Invalid(
            "only manual sessions or estimates can be created".into(),
        ));
    }
    if input.duration_minutes == 0 {
        return Err(AppError::Invalid("duration must be positive".into()));
    }
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    if note.as_ref().is_some_and(|n| n.chars().count() > 500) {
        return Err(AppError::Invalid("note is too long".into()));
    }

    let (started_at, ended_at) = if origin == SessionOrigin::Estimate {
        if input.duration_minutes > MAX_ESTIMATE_MINUTES {
            return Err(AppError::Invalid("estimate is too large".into()));
        }
        (None, None)
    } else {
        if input.duration_minutes > MAX_MANUAL_MINUTES {
            return Err(AppError::Invalid("a session cannot exceed 24 h".into()));
        }
        let raw = input
            .started_at
            .as_deref()
            .ok_or_else(|| AppError::Invalid("start date is required".into()))?;
        let start = DateTime::parse_from_rfc3339(raw)
            .map_err(|_| AppError::Invalid("invalid start date".into()))?
            .with_timezone(&Utc);
        let end = start + Duration::minutes(i64::from(input.duration_minutes));
        if end > now + Duration::minutes(5) {
            return Err(AppError::Invalid(
                "a session cannot end in the future".into(),
            ));
        }
        let fmt = |d: DateTime<Utc>| d.to_rfc3339_opts(SecondsFormat::Secs, true);
        (Some(fmt(start)), Some(fmt(end)))
    };

    Ok(PlaySession {
        id: existing.map(|s| s.id.clone()).unwrap_or_else(new_id),
        game_id: input.game_id.clone(),
        game_source_id: existing.and_then(|s| s.game_source_id.clone()),
        started_at,
        ended_at,
        duration_seconds: input.duration_minutes * 60,
        idle_seconds: existing.map(|s| s.idle_seconds).unwrap_or(0),
        origin,
        note,
        rating: existing.and_then(|s| s.rating),
    })
}

#[tauri::command]
#[specta::specta]
pub fn session_save(db: tauri::State<'_, Db>, input: SessionInput) -> CommandResult<PlaySession> {
    let conn = db.conn();
    games::get(&conn, &input.game_id)?;
    let existing = match &input.id {
        Some(id) => Some(sessions::get(&conn, id)?),
        None => None,
    };
    if existing
        .as_ref()
        .is_some_and(|s| s.game_id != input.game_id)
    {
        return Err(AppError::Invalid("session belongs to another game".into()).into());
    }
    let mut row = build_session(&input, existing.as_ref(), Utc::now())?;
    // New sessions of a single-source game are attached to that source, so the
    // cumul rule counts them there (ignored if the platform has its own counter).
    if row.game_source_id.is_none() {
        let sources = game_sources::list_for_game(&conn, &input.game_id)?;
        if let [only] = sources.as_slice() {
            row.game_source_id = Some(only.id.clone());
        }
    }
    sessions::save(&conn, &row)?;
    Ok(row)
}

#[tauri::command]
#[specta::specta]
pub fn session_delete(db: tauri::State<'_, Db>, id: String) -> CommandResult<()> {
    Ok(sessions::delete(&db.conn(), &id)?)
}

#[tauri::command]
#[specta::specta]
pub fn goals_list(db: tauri::State<'_, Db>, game_id: String) -> CommandResult<Vec<Goal>> {
    Ok(goals::list(&db.conn(), &game_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn goal_add(db: tauri::State<'_, Db>, game_id: String, title: String) -> CommandResult<Goal> {
    let conn = db.conn();
    games::get(&conn, &game_id)?;
    Ok(goals::add(&conn, &game_id, &title)?)
}

#[tauri::command]
#[specta::specta]
pub fn goal_toggle(db: tauri::State<'_, Db>, id: String, done: bool) -> CommandResult<()> {
    Ok(goals::set_done(&db.conn(), &id, done)?)
}

#[tauri::command]
#[specta::specta]
pub fn goal_rename(db: tauri::State<'_, Db>, id: String, title: String) -> CommandResult<()> {
    Ok(goals::rename(&db.conn(), &id, &title)?)
}

#[tauri::command]
#[specta::specta]
pub fn goal_delete(db: tauri::State<'_, Db>, id: String) -> CommandResult<()> {
    Ok(goals::delete(&db.conn(), &id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 5, 20, 0, 0).unwrap()
    }

    fn input(origin: SessionOrigin, start: Option<&str>, minutes: u32) -> SessionInput {
        SessionInput {
            id: None,
            game_id: "g".into(),
            started_at: start.map(Into::into),
            duration_minutes: minutes,
            origin,
            note: Some("  ".into()),
        }
    }

    #[test]
    fn manual_session_gets_an_end_date() {
        let s = build_session(
            &input(SessionOrigin::Manual, Some("2026-11-05T18:00:00+01:00"), 90),
            None,
            now(),
        )
        .unwrap();
        assert_eq!(s.started_at.as_deref(), Some("2026-11-05T17:00:00Z"));
        assert_eq!(s.ended_at.as_deref(), Some("2026-11-05T18:30:00Z"));
        assert_eq!(s.duration_seconds, 5400);
        assert_eq!(s.note, None);
    }

    #[test]
    fn estimates_have_no_dates() {
        let s = build_session(
            &input(SessionOrigin::Estimate, Some("2026-01-01T00:00:00Z"), 6000),
            None,
            now(),
        )
        .unwrap();
        assert_eq!((s.started_at, s.ended_at), (None, None));
        assert_eq!(s.origin, SessionOrigin::Estimate);
    }

    #[test]
    fn rejects_invalid_input() {
        let bad = [
            input(SessionOrigin::Manual, None, 10),
            input(SessionOrigin::Manual, Some("yesterday"), 10),
            input(SessionOrigin::Manual, Some("2026-11-05T19:00:00Z"), 0),
            input(SessionOrigin::Manual, Some("2026-11-05T19:30:00Z"), 60), // ends in the future
            input(SessionOrigin::Manual, Some("2026-11-01T00:00:00Z"), 25 * 60),
            input(SessionOrigin::Auto, Some("2026-11-05T10:00:00Z"), 10),
            input(SessionOrigin::Estimate, None, MAX_ESTIMATE_MINUTES + 1),
        ];
        for i in bad {
            assert!(build_session(&i, None, now()).is_err(), "{i:?}");
        }
    }

    #[test]
    fn editing_keeps_origin_source_and_idle() {
        let existing = PlaySession {
            id: "s1".into(),
            game_id: "g".into(),
            game_source_id: Some("steam".into()),
            started_at: Some("2026-11-05T10:00:00Z".into()),
            ended_at: Some("2026-11-05T11:00:00Z".into()),
            duration_seconds: 3600,
            idle_seconds: 300,
            origin: SessionOrigin::Auto,
            note: None,
            rating: None,
        };
        let mut edit = input(SessionOrigin::Manual, Some("2026-11-05T10:00:00Z"), 45);
        edit.id = Some("s1".into());
        edit.note = Some("Boss final".into());
        let s = build_session(&edit, Some(&existing), now()).unwrap();
        assert_eq!(s.id, "s1");
        assert_eq!(s.origin, SessionOrigin::Auto);
        assert_eq!(s.game_source_id.as_deref(), Some("steam"));
        assert_eq!(s.idle_seconds, 300);
        assert_eq!(s.duration_seconds, 2700);
        assert_eq!(s.note.as_deref(), Some("Boss final"));
    }
}
