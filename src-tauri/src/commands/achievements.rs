//! Achievements of a game (Steam for now), cached in SQLite.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::app::AppPaths;
use super::CommandResult;
use crate::db::repo::achievements::{self, AchievementRow};
use crate::db::repo::game_sources;
use crate::db::Db;
use crate::error::AppError;
use crate::metadata::achievement_icons;
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
    /// Emulated game not found on RetroAchievements (set its id by hand).
    NotMatched,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AchievementItem {
    pub api_name: String,
    pub name: String,
    pub description: Option<String>,
    pub unlocked_at: Option<String>,
    pub global_percent: Option<f64>,
    /// Icon in the image cache (`asset:`); `None` until downloaded.
    pub icon: Option<String>,
    /// Steam hidden achievement: description masked until unlocked or revealed.
    pub hidden: bool,
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
            // Remote URL here; `with_local_icons` swaps it for the cached file.
            icon: r.icon_url,
            hidden: r.hidden,
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
    let result = tauri::async_runtime::spawn_blocking(move || {
        let view = load(&app, &game_id, refresh)?;
        let (view, missing) = with_local_icons(&app, view);
        if !missing.is_empty() {
            fetch_icons_later(app.clone(), game_id, missing);
        }
        Ok::<_, super::CommandError>(view)
    })
    .await
    .map_err(|e| AppError::Invalid(format!("achievements task failed: {e}")))?;
    result
}

/// Emitted once the icons of a game are in the cache: the UI reloads them.
pub const ICONS_READY_EVENT: &str = "achievements-icons-ready";

/// Games whose icons are being downloaded (one download per game at a time).
static FETCHING: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);

/// Replaces remote icon URLs by their cached copy; returns the URLs to fetch.
/// Locked achievements show a padlock, so only unlocked icons are kept.
fn with_local_icons(
    app: &AppHandle,
    mut view: AchievementsView,
) -> (AchievementsView, Vec<String>) {
    let dir = achievement_icons::dir(&app.state::<AppPaths>().data_dir);
    let mut missing = Vec::new();
    for item in &mut view.items {
        let Some(url) = item.icon.take() else {
            continue;
        };
        if item.unlocked_at.is_none() {
            continue;
        }
        match achievement_icons::cached(&dir, &url) {
            Some(path) => item.icon = Some(path.display().to_string()),
            None => missing.push(url),
        }
    }
    (view, missing)
}

/// Downloads the missing icons in the background, then tells the UI.
fn fetch_icons_later(app: AppHandle, game_id: String, urls: Vec<String>) {
    {
        let mut guard = FETCHING.lock().unwrap_or_else(|e| e.into_inner());
        if !guard
            .get_or_insert_with(Default::default)
            .insert(game_id.clone())
        {
            return;
        }
    }
    std::thread::spawn(move || {
        let dir = achievement_icons::dir(&app.state::<AppPaths>().data_dir);
        let added = achievement_icons::fetch_missing(&dir, &urls);
        if let Some(set) = FETCHING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            set.remove(&game_id);
        }
        if added > 0 {
            let _ = app.emit(ICONS_READY_EVENT, &game_id);
        }
    });
}

/// Icons of a game's achievements, downloaded now (end-of-session recap).
pub fn fetch_icons_now(app: &AppHandle, game_id: &str) {
    if let Ok(view) = load(app, game_id, false) {
        let (_, missing) = with_local_icons(app, view);
        let dir = achievement_icons::dir(&app.state::<AppPaths>().data_dir);
        achievement_icons::fetch_missing(&dir, &missing);
    }
}

/// Refetches the achievements of a game (end of session).
pub fn refresh(app: &AppHandle, game_id: &str) -> CommandResult<AchievementsView> {
    load(app, game_id, true)
}

fn load(app: &AppHandle, game_id: &str, refresh: bool) -> CommandResult<AchievementsView> {
    let db = app.state::<Db>();
    let sources = game_sources::list_for_game(&db.conn(), game_id)?;
    if let Some(src) = sources.iter().find(|s| s.kind == SourceKind::Emulator) {
        return load_ra(app, src, refresh);
    }
    let Some(source) = sources.iter().find(|s| s.kind == SourceKind::Steam) else {
        // Other platforms (GOG) fill the cache during the sync.
        let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();
        return Ok(
            cached_view(&db.conn(), &ids)?.unwrap_or_else(|| empty(AchievementsState::Unsupported))
        );
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
                        hidden: a.hidden,
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

/// Cached list of the first source that has one (GOG fills it during the sync).
/// Takes one connection for both queries: locking `Db` twice in the same
/// `if let` (its guard lives until the end of the block) froze the app.
fn cached_view(
    conn: &rusqlite::Connection,
    source_ids: &[&str],
) -> crate::error::AppResult<Option<AchievementsView>> {
    for id in source_ids {
        if let Some((synced_at, _)) = achievements::sync_state(conn, id)? {
            let rows = achievements::list_for_source(conn, id)?;
            return Ok(Some(view(rows, Some(synced_at), None)));
        }
    }
    Ok(None)
}

/// RetroAchievements for an emulated game, cached like Steam's.
fn load_ra(
    app: &AppHandle,
    source: &crate::db::repo::game_sources::GameSource,
    refresh: bool,
) -> CommandResult<AchievementsView> {
    use super::emulators::{fetch_ra, store_ra, RaResult};
    let db = app.state::<Db>();
    let cached = achievements::sync_state(&db.conn(), &source.id)?;
    if let Some((synced_at, _)) = &cached {
        if !refresh && is_fresh(synced_at) {
            let rows = achievements::list_for_source(&db.conn(), &source.id)?;
            return Ok(view(rows, Some(synced_at.clone()), None));
        }
    }
    match fetch_ra(app, source) {
        Ok(RaResult::Rows(rows)) => {
            store_ra(&db, &source.id, &rows)?;
            let synced = achievements::sync_state(&db.conn(), &source.id)?.map(|(s, _)| s);
            Ok(view(
                achievements::list_for_source(&db.conn(), &source.id)?,
                synced,
                None,
            ))
        }
        Ok(RaResult::NeedsKey) => Ok(empty(AchievementsState::NeedsKey)),
        Ok(RaResult::NotMatched) => Ok(empty(AchievementsState::NotMatched)),
        Err(e) => {
            tracing::warn!(error = %e, "RetroAchievements fetch failed");
            let rows = achievements::list_for_source(&db.conn(), &source.id)?;
            Ok(view(rows, cached.map(|(s, _)| s), Some(e.to_string())))
        }
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::repo::games;

    #[test]
    fn gog_achievements_come_from_the_cache() {
        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let game = games::insert(&conn, "The Witcher 3").unwrap();
        let gog = game_sources::upsert(
            &conn,
            &game.id,
            SourceKind::Gog,
            &SourceData {
                external_id: Some("1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(cached_view(&conn, &[&gog.id]).unwrap().is_none());

        let row = |name: &str, unlocked: Option<&str>| AchievementRow {
            api_name: name.into(),
            name: Some(name.into()),
            description: None,
            icon_url: None,
            unlocked_at: unlocked.map(Into::into),
            global_percent: Some(30.0),
            hidden: false,
        };
        achievements::replace_for_source(
            &mut conn,
            &gog.id,
            Some(&[
                row("ALL_IN", None),
                row("BOOKWORM", Some("2016-11-03T11:35:52Z")),
            ]),
        )
        .unwrap();
        let v = cached_view(&conn, &[&gog.id]).unwrap().unwrap();
        assert_eq!(
            (v.state, v.unlocked, v.total),
            (AchievementsState::Ok, 1, 2)
        );
    }
}
