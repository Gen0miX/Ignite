//! Game sheet media: Steam Store trailers and screenshots, and the game's news.
//! Games owned elsewhere (Epic, GOG…) use the Steam page with the same title;
//! what Steam lacks comes from IGDB when its keys are set.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::{game_sources, games};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::metadata::igdb::{self, IgdbMedia};
use crate::metadata::news::{self, NewsItem};
use crate::metadata::steam_store::{self, StoreMedia};
use crate::models::SourceKind;
use crate::secrets::{SecretKind, Secrets};

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
    /// Part of the media (or all of it) comes from IGDB.
    pub from_igdb: bool,
    /// Steam lacks something IGDB could give, but the IGDB keys are not set.
    pub igdb_keys_missing: bool,
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

struct GameInfo {
    title: String,
    igdb_id: Option<i64>,
    /// App id of the Steam source, when the game is owned on Steam.
    owned_appid: Option<String>,
}

fn game_info(app: &AppHandle, game_id: &str) -> AppResult<GameInfo> {
    let db = app.state::<Db>();
    let conn = db.conn();
    let game = games::get(&conn, game_id)?;
    let owned_appid = game_sources::list_for_game(&conn, game_id)?
        .into_iter()
        .find(|s| s.kind == SourceKind::Steam)
        .and_then(|s| s.external_id);
    Ok(GameInfo {
        title: game.title,
        igdb_id: game.igdb_id,
        owned_appid,
    })
}

/// `(appid, found by title)`.
fn steam_appid(game_id: &str, info: &GameInfo) -> AppResult<(Option<String>, bool)> {
    if info.owned_appid.is_some() {
        return Ok((info.owned_appid.clone(), false));
    }
    if let Some(found) = cached(&APPIDS, game_id, TTL) {
        return Ok((found, true));
    }
    let found = steam_store::search_appid(&info.title)?;
    store(&APPIDS, game_id, found.clone());
    Ok((found, true))
}

type SteamResult = (Option<String>, bool, StoreMedia);

fn steam_media(game_id: &str, info: &GameInfo) -> AppResult<SteamResult> {
    let (appid, by_title) = steam_appid(game_id, info)?;
    let media = match &appid {
        Some(id) => steam_store::fetch_details(id)?.unwrap_or_default(),
        None => StoreMedia::default(),
    };
    Ok((appid, by_title, media))
}

/// Fills what Steam lacks (trailers, screenshots, description) from IGDB.
/// Returns whether IGDB content was used.
fn fill_from_igdb(media: &mut StoreMedia, found: IgdbMedia) -> bool {
    let mut used = false;
    if media.trailers.is_empty() && !found.trailers.is_empty() {
        media.trailers = found.trailers;
        used = true;
    }
    if media.screenshots.is_empty() && !found.screenshots.is_empty() {
        media.screenshots = found.screenshots;
        used = true;
    }
    if media.description.is_none() && found.summary.is_some() {
        media.description = found.summary;
        used = true;
    }
    used
}

fn igdb_keys(app: &AppHandle) -> AppResult<Option<(String, String)>> {
    let secrets = app.state::<Secrets>();
    Ok(secrets
        .get(SecretKind::IgdbClientId)?
        .zip(secrets.get(SecretKind::IgdbClientSecret)?))
}

fn load_media(app: &AppHandle, game_id: &str) -> AppResult<GameMedia> {
    let info = game_info(app, game_id)?;
    // A Steam failure does not hide what IGDB can show.
    let (steam_appid, by_title, mut media, steam_error) = match steam_media(game_id, &info) {
        Ok((appid, by_title, media)) => (appid, by_title, media, None),
        Err(e) => (None, false, StoreMedia::default(), Some(e)),
    };
    let needs_igdb = media.trailers.is_empty() || media.screenshots.is_empty();
    let mut from_igdb = false;
    let mut igdb_keys_missing = false;
    if needs_igdb {
        match igdb_keys(app)? {
            None => igdb_keys_missing = true,
            Some((id, secret)) => match igdb::fetch(&id, &secret, info.igdb_id, &info.title) {
                Ok(Some(found)) => {
                    if info.igdb_id != Some(found.id) {
                        let db = app.state::<Db>();
                        db.conn().execute(
                            "UPDATE games SET igdb_id = ?2 WHERE id = ?1",
                            rusqlite::params![game_id, found.id],
                        )?;
                    }
                    from_igdb = fill_from_igdb(&mut media, found);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "IGDB media fetch failed"),
            },
        }
    }
    if let Some(e) = steam_error {
        if !from_igdb {
            return Err(e);
        }
        tracing::warn!(error = %e, "Steam media fetch failed");
    }
    Ok(GameMedia {
        steam_appid,
        by_title,
        from_igdb,
        igdb_keys_missing,
        media,
    })
}

/// Trailers, screenshots and short description: Steam Store first, IGDB
/// for what Steam lacks.
#[tauri::command]
#[specta::specta]
pub async fn game_store_media(app: AppHandle, game_id: String) -> CommandResult<GameMedia> {
    if let Some(hit) = cached(&MEDIA, &game_id, TTL) {
        return Ok(hit);
    }
    let result = tauri::async_runtime::spawn_blocking(move || -> AppResult<GameMedia> {
        let out = load_media(&app, &game_id)?;
        // Not cached while IGDB keys are missing: once added, they apply at once.
        if !out.igdb_keys_missing {
            store(&MEDIA, &game_id, out.clone());
        }
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
        let info = game_info(&app, &game_id)?;
        let (appid, _) = steam_appid(&game_id, &info)?;
        let (items, hidden) = match &appid {
            Some(id) => news::fetch(id, &game_id, &info.title, NEWS_COUNT)?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::steam_store::{Screenshot, Trailer};

    fn shot(n: &str) -> Screenshot {
        Screenshot {
            thumb: n.into(),
            full: n.into(),
        }
    }

    #[test]
    fn igdb_only_fills_what_steam_lacks() {
        let mut media = StoreMedia {
            description: Some("Steam".into()),
            trailers: vec![],
            screenshots: vec![shot("steam")],
        };
        let found = IgdbMedia {
            id: 1,
            summary: Some("IGDB".into()),
            trailers: vec![Trailer {
                name: "T".into(),
                thumbnail: "t".into(),
                hls: None,
                youtube: Some("91t0ha9x0AE".into()),
            }],
            screenshots: vec![shot("igdb")],
        };
        assert!(fill_from_igdb(&mut media, found));
        assert_eq!(media.description.as_deref(), Some("Steam"));
        assert_eq!(media.screenshots, vec![shot("steam")]);
        assert_eq!(media.trailers.len(), 1);

        let mut full = media.clone();
        assert!(!fill_from_igdb(&mut full, IgdbMedia::default()));
    }
}
