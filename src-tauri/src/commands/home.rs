//! Home screen commands.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::game_sources;
use crate::db::Db;
use crate::error::AppError;
use crate::library::couch::{self, Mood, Shelf};
use crate::library::home::{self, HomeGame, HomeOverview, Tonight};
use crate::library::query;
use crate::metadata::news::{self, NewsItem};
use crate::models::SourceKind;

#[tauri::command]
#[specta::specta]
pub fn home_overview(db: tauri::State<'_, Db>) -> CommandResult<HomeOverview> {
    Ok(home::overview(&db.conn())?)
}

#[tauri::command]
#[specta::specta]
pub fn home_tonight(db: tauri::State<'_, Db>, choice: Tonight) -> CommandResult<Vec<HomeGame>> {
    Ok(home::tonight(&db.conn(), choice, chrono::Utc::now())?)
}

/// Games of a couch-mode shelf.
#[tauri::command]
#[specta::specta]
pub fn couch_shelf(db: tauri::State<'_, Db>, shelf: Shelf) -> CommandResult<Vec<HomeGame>> {
    Ok(couch::shelf(&db.conn(), shelf)?)
}

/// Games the random wheel may land on for a mood.
#[tauri::command]
#[specta::specta]
pub fn couch_wheel(db: tauri::State<'_, Db>, mood: Mood) -> CommandResult<Vec<HomeGame>> {
    Ok(couch::wheel(&db.conn(), mood)?)
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct HomeNews {
    pub items: Vec<NewsItem>,
    /// Promotions and press articles, shown on demand.
    pub hidden: Vec<NewsItem>,
}

/// News are fetched at most every 30 minutes.
static NEWS_CACHE: Mutex<Option<(Instant, HomeNews)>> = Mutex::new(None);
const NEWS_TTL: Duration = Duration::from_secs(30 * 60);
const NEWS_GAMES: usize = 8;

/// Steam news of the games played in the last 30 days.
#[tauri::command]
#[specta::specta]
pub async fn home_news(app: AppHandle) -> CommandResult<HomeNews> {
    if let Some((at, cached)) = NEWS_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        if at.elapsed() < NEWS_TTL {
            return Ok(cached.clone());
        }
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> CommandResult<HomeNews> {
        let db = app.state::<Db>();
        let targets: Vec<(String, String, String, Option<String>)> = {
            let conn = db.conn();
            let since = chrono::Utc::now() - chrono::Duration::days(30);
            let mut recent: Vec<_> = query::summaries(&conn)?
                .into_iter()
                .filter(|g| !g.hidden)
                .filter(|g| {
                    g.last_played
                        .as_deref()
                        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                        .is_some_and(|d| d > since)
                })
                .collect();
            recent.sort_by(|a, b| b.last_played.cmp(&a.last_played));
            let mut out = Vec::new();
            for g in recent {
                let steam = game_sources::list_for_game(&conn, &g.id)?
                    .into_iter()
                    .find(|s| s.kind == SourceKind::Steam)
                    .and_then(|s| s.external_id);
                if let Some(appid) = steam {
                    out.push((appid, g.id, g.title, g.header));
                }
                if out.len() >= NEWS_GAMES {
                    break;
                }
            }
            out
        };
        let mut items = Vec::new();
        let mut hidden = Vec::new();
        for (appid, game_id, title, header) in targets {
            match news::fetch(&appid, &game_id, &title, 5) {
                Ok((kept, promo)) => {
                    let with_header = |mut n: NewsItem| {
                        n.header = header.clone();
                        n
                    };
                    items.extend(kept.into_iter().map(with_header));
                    hidden.extend(promo.into_iter().map(with_header));
                }
                Err(e) => tracing::debug!(%appid, error = %e, "news fetch failed"),
            }
        }
        items.sort_by(|a, b| b.date.cmp(&a.date));
        items.truncate(6);
        hidden.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(HomeNews { items, hidden })
    })
    .await
    .map_err(|e| AppError::Invalid(format!("news task failed: {e}")))??;
    *NEWS_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), result.clone()));
    Ok(result)
}

/// Opens a news article in the default browser (https only).
#[tauri::command]
#[specta::specta]
pub fn open_link(url: String) -> CommandResult<()> {
    if !url.starts_with("https://") {
        return Err(AppError::Invalid("only https links can be opened".into()).into());
    }
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|e| AppError::Invalid(format!("cannot open link: {e}")).into())
}
