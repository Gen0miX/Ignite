//! Artwork choice from SteamGridDB (covers, heroes, logos).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::app::AppPaths;
use super::library::{load_detail, GameDetail};
use super::CommandResult;
use crate::db::repo::artwork::{self, ArtworkRow};
use crate::db::repo::{game_sources, games};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::metadata::artwork::{cache_root, download_named};
use crate::metadata::steamgriddb::{self as sgdb, ImageKind};
use crate::models::SourceKind;
use crate::secrets::{SecretKind, Secrets};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ArtworkSlot {
    Cover,
    Hero,
    Logo,
}

impl ArtworkSlot {
    fn kind(self) -> ImageKind {
        match self {
            ArtworkSlot::Cover => ImageKind::Cover,
            ArtworkSlot::Hero => ImageKind::Hero,
            ArtworkSlot::Logo => ImageKind::Logo,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            ArtworkSlot::Cover => "cover",
            ArtworkSlot::Hero => "hero",
            ArtworkSlot::Logo => "logo",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ArtworkSearchState {
    Ok,
    /// No SteamGridDB key in the vault.
    NeedsKey,
    /// No game found for this title.
    NoMatch,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SgdbGameOption {
    pub id: u32,
    pub name: String,
    pub year: Option<u32>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkOption {
    pub url: String,
    /// Local copy of the thumbnail (served via `asset:`), when downloaded.
    pub thumb: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkSearch {
    pub state: ArtworkSearchState,
    /// Matching SteamGridDB games, to pick another one if the guess is wrong.
    pub games: Vec<SgdbGameOption>,
    pub selected_game: Option<u32>,
    pub options: Vec<ArtworkOption>,
}

/// Thumbnails shown in the picker (more = slower; they are downloaded).
const MAX_OPTIONS: usize = 18;

fn key(app: &AppHandle) -> AppResult<Option<String>> {
    app.state::<Secrets>().get(SecretKind::SteamGridDbApiKey)
}

fn year(ts: Option<i64>) -> Option<u32> {
    use chrono::Datelike;
    ts.and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.year() as u32)
}

fn search_blocking(
    app: &AppHandle,
    game_id: &str,
    slot: ArtworkSlot,
    sgdb_game_id: Option<u32>,
    query: Option<String>,
) -> AppResult<ArtworkSearch> {
    let Some(key) = key(app)? else {
        return Ok(ArtworkSearch {
            state: ArtworkSearchState::NeedsKey,
            games: vec![],
            selected_game: None,
            options: vec![],
        });
    };
    let db = app.state::<Db>();
    let (game, steam_appid) = {
        let conn = db.conn();
        let game = games::get(&conn, game_id)?;
        let appid = game_sources::list_for_game(&conn, game_id)?
            .into_iter()
            .find(|s| s.kind == SourceKind::Steam)
            .and_then(|s| s.external_id);
        (game, appid)
    };

    let term = query
        .filter(|q| !q.trim().is_empty())
        .unwrap_or_else(|| game.title.clone());
    let found = sgdb::search(&key, &term)?;
    let games: Vec<SgdbGameOption> = found
        .iter()
        .take(10)
        .map(|g| SgdbGameOption {
            id: g.id,
            name: g.name.clone(),
            year: year(g.release_date),
        })
        .collect();

    let selected = match sgdb_game_id {
        Some(id) => Some(id),
        None => match game.steamgriddb_id.and_then(|id| u32::try_from(id).ok()) {
            Some(id) => Some(id),
            None => match steam_appid {
                Some(appid) => sgdb::by_steam_appid(&key, &appid)?.map(|g| g.id),
                None => None,
            }
            .or_else(|| found.first().map(|g| g.id)),
        },
    };
    let Some(selected) = selected else {
        return Ok(ArtworkSearch {
            state: ArtworkSearchState::NoMatch,
            games,
            selected_game: None,
            options: vec![],
        });
    };

    let thumbs_dir = cache_root(&app.state::<AppPaths>().data_dir).join("_sgdb");
    let options = sgdb::images(&key, selected, slot.kind())?
        .into_iter()
        .filter(|i| sgdb::is_sgdb_url(&i.url) && sgdb::is_sgdb_url(&i.thumb))
        .take(MAX_OPTIONS)
        .map(|i| {
            let thumb = download_named(
                &thumbs_dir,
                &format!("{}-{}", slot.as_str(), i.id),
                &i.thumb,
            )
            .map_err(|e| tracing::debug!(error = %e, "thumbnail download failed"))
            .ok()
            .map(|p| p.display().to_string());
            ArtworkOption {
                url: i.url,
                thumb,
                width: i.width,
                height: i.height,
                style: i.style,
            }
        })
        .collect();
    Ok(ArtworkSearch {
        state: ArtworkSearchState::Ok,
        games,
        selected_game: Some(selected),
        options,
    })
}

/// Finds artwork options for a game on SteamGridDB.
#[tauri::command]
#[specta::specta]
pub async fn artwork_search(
    app: AppHandle,
    game_id: String,
    slot: ArtworkSlot,
    sgdb_game_id: Option<u32>,
    query: Option<String>,
) -> CommandResult<ArtworkSearch> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        search_blocking(&app, &game_id, slot, sgdb_game_id, query)
    })
    .await
    .map_err(|e| AppError::Invalid(format!("artwork task failed: {e}")))?;
    Ok(result?)
}

fn set_blocking(
    app: &AppHandle,
    game_id: &str,
    slot: ArtworkSlot,
    url: &str,
    sgdb_game_id: Option<u32>,
) -> AppResult<()> {
    if !sgdb::is_sgdb_url(url) {
        return Err(AppError::Invalid(
            "only SteamGridDB images can be used".into(),
        ));
    }
    let db = app.state::<Db>();
    games::get(&db.conn(), game_id)?;
    let dir = cache_root(&app.state::<AppPaths>().data_dir).join(game_id);
    let previous = artwork::for_game(&db.conn(), game_id)?.remove(slot.as_str());
    // A new file name each time, so the webview never shows a cached old image.
    let name = format!("{}-sgdb-{}", slot.as_str(), &crate::db::new_id()[..8]);
    let path = download_named(&dir, &name, url)?;
    {
        let conn = db.conn();
        artwork::upsert(
            &conn,
            &ArtworkRow {
                game_id: game_id.to_string(),
                kind: slot.as_str().to_string(),
                local_path: path.display().to_string(),
                provider: Some("steamgriddb".into()),
                source_url: Some(url.to_string()),
            },
        )?;
        if let Some(id) = sgdb_game_id {
            conn.execute(
                "UPDATE games SET steamgriddb_id = ?2 WHERE id = ?1",
                rusqlite::params![game_id, i64::from(id)],
            )?;
        }
    }
    if let Some(old) = previous.filter(|p| *p != path.display().to_string()) {
        // Only delete files inside our own cache.
        if std::path::Path::new(&old).starts_with(&dir) {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(())
}

/// Downloads the chosen image into the cache and uses it for the game.
#[tauri::command]
#[specta::specta]
pub async fn artwork_set(
    app: AppHandle,
    game_id: String,
    slot: ArtworkSlot,
    url: String,
    sgdb_game_id: Option<u32>,
) -> CommandResult<GameDetail> {
    let app2 = app.clone();
    let id = game_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        set_blocking(&app2, &id, slot, &url, sgdb_game_id)
    })
    .await
    .map_err(|e| AppError::Invalid(format!("artwork task failed: {e}")))??;
    load_detail(&app.state::<Db>(), &game_id)
}
