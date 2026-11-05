//! Achievements of a game (Steam for now), cached in SQLite.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::achievements::{self, AchievementRow};
use crate::db::repo::game_sources;
use crate::db::Db;
use crate::error::AppError;
use crate::models::SourceKind;
use crate::secrets::Secrets;
use crate::sources::steam::achievements as steam;
use crate::sources::unix_to_rfc3339;

/// Cached data younger than this is not refetched (INTEGRATIONS.md: cache).
const FRESH_HOURS: i64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AchievementsState {
    /// List available.
    Ok,
    /// The platform says this game has no achievements.
    None,
    /// A Steam Web API key (and SteamID) is needed.
    NeedsKey,
    /// No source of this game provides achievements: use personal goals.
    Unsupported,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AchievementItem {
    pub api_name: String,
    pub name: String,
    pub description: Option<String>,
    pub unlocked_at: Option<String>,
    pub global_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsView {
    pub state: AchievementsState,
    pub unlocked: u32,
    pub total: u32,
    pub items: Vec<AchievementItem>,
    pub synced_at: Option<String>,
    /// Last fetch error (cached data, if any, is still returned).
    pub error: Option<String>,
}

fn view(
    rows: Vec<AchievementRow>,
    synced_at: Option<String>,
    error: Option<String>,
) -> AchievementsView {
    let items: Vec<AchievementItem> = rows
        .into_iter()
        .map(|r| AchievementItem {
            name: r.name.clone().unwrap_or_else(|| r.api_name.clone()),
            api_name: r.api_name,
            description: r.description,
            unlocked_at: r.unlocked_at,
            global_percent: r.global_percent,
        })
        .collect();
    AchievementsView {
        state: if items.is_empty() {
            AchievementsState::None
        } else {
            AchievementsState::Ok
        },
        unlocked: items.iter().filter(|i| i.unlocked_at.is_some()).count() as u32,
        total: items.len() as u32,
        items,
        synced_at,
        error,
    }
}

fn empty(state: AchievementsState) -> AchievementsView {
    AchievementsView {
        state,
        unlocked: 0,
        total: 0,
        items: vec![],
        synced_at: None,
        error: None,
    }
}

fn is_fresh(synced_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(synced_at)
        .map(|d| chrono::Utc::now().signed_duration_since(d).num_hours() < FRESH_HOURS)
        .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub async fn achievements_get(
    app: AppHandle,
    game_id: String,
    refresh: bool,
) -> CommandResult<AchievementsView> {
    let result = tauri::async_runtime::spawn_blocking(move || load(&app, &game_id, refresh))
        .await
        .map_err(|e| AppError::Invalid(format!("achievements task failed: {e}")))?;
    result
}

fn load(app: &AppHandle, game_id: &str, refresh: bool) -> CommandResult<AchievementsView> {
    let db = app.state::<Db>();
    let sources = game_sources::list_for_game(&db.conn(), game_id)?;
    let Some(source) = sources.iter().find(|s| s.kind == SourceKind::Steam) else {
        // Other platforms (GOG) fill the cache during the sync.
        for s in &sources {
            if let Some((synced_at, _)) = achievements::sync_state(&db.conn(), &s.id)? {
                let rows = achievements::list_for_source(&db.conn(), &s.id)?;
                return Ok(view(rows, Some(synced_at), None));
            }
        }
        return Ok(empty(AchievementsState::Unsupported));
    };
    let appid: u64 = source
        .external_id
        .as_deref()
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| AppError::Invalid("Steam source without appid".into()))?;

    let cached = achievements::sync_state(&db.conn(), &source.id)?;
    if let Some((synced_at, _)) = &cached {
        if !refresh && is_fresh(synced_at) {
            let rows = achievements::list_for_source(&db.conn(), &source.id)?;
            return Ok(view(rows, Some(synced_at.clone()), None));
        }
    }

    let config = super::sources::steam_config(&db, &app.state::<Secrets>());
    let (Some(key), Some(steam_id)) = (config.api_key, config.steam_id) else {
        return match cached {
            Some((synced_at, _)) => Ok(view(
                achievements::list_for_source(&db.conn(), &source.id)?,
                Some(synced_at),
                None,
            )),
            None => Ok(empty(AchievementsState::NeedsKey)),
        };
    };

    match steam::fetch(&key, steam_id, appid) {
        Ok(list) => {
            let rows: Option<Vec<AchievementRow>> = list.map(|l| {
                l.into_iter()
                    .map(|a| AchievementRow {
                        api_name: a.api_name,
                        name: a.name,
                        description: a.description,
                        icon_url: a.icon_url,
                        unlocked_at: a.unlocked_at.and_then(unix_to_rfc3339),
                        global_percent: a.global_percent,
                    })
                    .collect()
            });
            achievements::replace_for_source(&mut db.conn(), &source.id, rows.as_deref())?;
            let synced = achievements::sync_state(&db.conn(), &source.id)?.map(|(s, _)| s);
            Ok(view(
                achievements::list_for_source(&db.conn(), &source.id)?,
                synced,
                None,
            ))
        }
        Err(e) => {
            tracing::warn!(game = %game_id, error = %e, "achievements fetch failed");
            let rows = achievements::list_for_source(&db.conn(), &source.id)?;
            let mut v = view(rows, cached.map(|(s, _)| s), Some(e.to_string()));
            if v.total == 0 && v.synced_at.is_none() {
                v.state = AchievementsState::Ok;
            }
            Ok(v)
        }
    }
}
