//! Tracking rules (Settings › Sources & suivi): minimum session, idle pause.

use serde::{Deserialize, Serialize};

use super::CommandResult;
use crate::db::repo::settings;
use crate::db::Db;
use crate::error::AppError;
use crate::tracker::{load_config, IDLE_KEY, MIN_SESSION_KEY};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackingSettings {
    /// Sessions shorter than this are discarded (0 = keep everything).
    pub min_session_minutes: u32,
    /// No input for this long pauses the timer.
    pub idle_minutes: u32,
    /// Windows toast when a Goldberg achievement is unlocked.
    pub achievement_toasts: bool,
}

pub fn validate(s: &TrackingSettings) -> Result<(), AppError> {
    if s.min_session_minutes > 30 {
        return Err(AppError::Invalid(
            "minimum session must be 0-30 minutes".into(),
        ));
    }
    if !(1..=240).contains(&s.idle_minutes) {
        return Err(AppError::Invalid("idle pause must be 1-240 minutes".into()));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tracking_settings_get(db: tauri::State<'_, Db>) -> TrackingSettings {
    let config = load_config(&db);
    TrackingSettings {
        min_session_minutes: (config.min_session_secs / 60) as u32,
        idle_minutes: (config.idle_threshold_secs / 60) as u32,
        achievement_toasts: settings::get::<bool>(&db.conn(), crate::sources::goldberg::NOTIFY_KEY)
            .ok()
            .flatten()
            .unwrap_or(true),
    }
}

/// Saved settings are picked up by the tracker within 30 seconds.
#[tauri::command]
#[specta::specta]
pub fn tracking_settings_set(
    db: tauri::State<'_, Db>,
    settings: TrackingSettings,
) -> CommandResult<()> {
    validate(&settings)?;
    let conn = db.conn();
    settings::set(
        &conn,
        MIN_SESSION_KEY,
        &i64::from(settings.min_session_minutes),
    )?;
    settings::set(&conn, IDLE_KEY, &i64::from(settings.idle_minutes))?;
    settings::set(
        &conn,
        crate::sources::goldberg::NOTIFY_KEY,
        &settings.achievement_toasts,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn validates_and_round_trips() {
        assert!(validate(&TrackingSettings {
            min_session_minutes: 31,
            idle_minutes: 15,
            achievement_toasts: true,
        })
        .is_err());
        assert!(validate(&TrackingSettings {
            min_session_minutes: 2,
            idle_minutes: 0,
            achievement_toasts: true,
        })
        .is_err());
        let db = Db::open_in_memory().unwrap();
        // Defaults from the SPEC.
        let c = load_config(&db);
        assert_eq!((c.min_session_secs, c.idle_threshold_secs), (120, 900));
        settings::set(&db.conn(), MIN_SESSION_KEY, &5i64).unwrap();
        settings::set(&db.conn(), IDLE_KEY, &30i64).unwrap();
        let c = load_config(&db);
        assert_eq!((c.min_session_secs, c.idle_threshold_secs), (300, 1800));
    }
}
