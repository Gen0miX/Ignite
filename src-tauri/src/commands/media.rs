//! Game sheet media: Steam Store trailers and screenshots, and the game's news.
//! Games owned elsewhere (Epic, GOG…) use the Steam page with the same title.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::{game_sources, games};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::metadata::news::{self, NewsItem};
use crate::metadata::steam_store::{self, StoreMedia};
use crate::models::SourceKind;

const TTL: Duration = Duration::from_secs(6 * 60 * 60);
const NEWS_TTL: Duration = Duration::from_secs(30 * 60);
const NEWS_COUNT: u32 = 10;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameMedia {
    /// Steam app the media come from; `None` when no Steam page was found.
    pub steam_appid: Option<String>,
    /// The page was found by title (game not owned on Steam).
    pub by_title: bool,
    pub media: StoreMedia,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct GameNews {
    pub items: Vec<NewsItem>,
    /// Promotions and press articles, shown on demand.
    pub hidden: Vec<NewsItem>,
}

type Cache<T> = Mutex<Option<HashMap<String, (Instant, T)>>>;

static APPIDS: Cache<Option<String>> = Mutex::new(None);
static MEDIA: Cache<GameMedia> = Mutex::new(None);
static NEWS: Cache<GameNews> = Mutex::new(None);

fn cached<T: Clone>(cache: &Cache<T>, key: &str, ttl: Duration) -> Option<T> {
    let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let (at, value) = guard.as_ref()?.get(key)?;
    (at.elapsed() < ttl).then(|| value.clone())
}

fn store<T>(cache: &Cache<T>, key: &str, value: T) {
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(key.to_string(), (Instant::now(), value));
}

/// `(appid, found by title, game title)`.
fn steam_appid(app: &AppHandle, game_id: &str) -> AppResult<(Option<String>, bool, String)> {
    let (owned, title) = {
        let db = app.state::<Db>();
        let conn = db.conn();
        let title = games::get(&conn, game_id)?.title;
        let owned = game_sources::list_for_game(&conn, game_id)?
            .into_iter()
            .find(|s| s.kind == SourceKind::Steam)
            .and_then(|s| s.external_id);
        (owned, title)
    };
    if owned.is_some() {
        return Ok((owned, false, title));
    }
    if let Some(found) = cached(&APPIDS, game_id, TTL) {
        return Ok((found, true, title));
    }
    let found = steam_store::search_appid(&title)?;
    store(&APPIDS, game_id, found.clone());
    Ok((found, true, title))
}

/// Trailers, screenshots and short description from the Steam Store.
#[tauri::command]
#[specta::specta]
pub async fn game_store_media(app: AppHandle, game_id: String) -> CommandResult<GameMedia> {
    if let Some(hit) = cached(&MEDIA, &game_id, TTL) {
        return Ok(hit);
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> AppResult<GameMedia> {
        let (appid, by_title, _) = steam_appid(&app, &game_id)?;
        let media = match &appid {
            Some(id) => steam_store::fetch_details(id)?.unwrap_or_default(),
            None => StoreMedia::default(),
        };
        let out = GameMedia {
            steam_appid: appid,
            by_title,
            media,
        };
        store(&MEDIA, &game_id, out.clone());
        Ok(out)
    })
    .await
    .map_err(|e| AppError::Invalid(format!("media task failed: {e}")))??;
    Ok(result)
}

/// Latest Steam news of the game (developer announcements first).
#[tauri::command]
#[specta::specta]
pub async fn game_news(app: AppHandle, game_id: String) -> CommandResult<GameNews> {
    if let Some(hit) = cached(&NEWS, &game_id, NEWS_TTL) {
        return Ok(hit);
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> AppResult<GameNews> {
        let (appid, _, title) = steam_appid(&app, &game_id)?;
        let (items, hidden) = match &appid {
            Some(id) => news::fetch(id, &game_id, &title, NEWS_COUNT)?,
            None => (vec![], vec![]),
        };
        let out = GameNews { items, hidden };
        store(&NEWS, &game_id, out.clone());
        Ok(out)
    })
    .await
    .map_err(|e| AppError::Invalid(format!("news task failed: {e}")))??;
    Ok(result)
}
