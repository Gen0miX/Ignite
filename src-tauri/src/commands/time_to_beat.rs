//! Time to beat of a game (docs/TIME-TO-BEAT.md): IGDB `game_time_to_beats`,
//! cached 30 days, and the user's correction, which always wins.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::CommandResult;
use crate::db::repo::games;
use crate::db::repo::time_to_beat::{self as ttb, TimeToBeat, TtbProvider};
use crate::db::{now_rfc3339, Db};
use crate::error::{AppError, AppResult};
use crate::metadata::igdb::{self, IgdbCandidate, Match};
use crate::secrets::{SecretKind, Secrets};

/// IGDB values are fetched again after this many days.
const FRESH_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TimeToBeatView {
    /// Manual correction where set, else IGDB; `None` when unknown.
    pub best: Option<TimeToBeat>,
    pub sources: Vec<TimeToBeat>,
    /// IGDB games to choose from when the title match was not exact.
    pub candidates: Vec<IgdbCandidate>,
    pub igdb_keys_missing: bool,
    /// Last fetch failed (offline…); cached values are still returned.
    pub error: Option<String>,
}

/// Candidates of games without an exact match, for this run of the app (no
/// new search each time the sheet opens).
static CANDIDATES: Mutex<Option<HashMap<String, Vec<IgdbCandidate>>>> = Mutex::new(None);

fn candidates_for(game_id: &str) -> Option<Vec<IgdbCandidate>> {
    CANDIDATES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(game_id).cloned())
}

fn remember_candidates(game_id: &str, list: Option<Vec<IgdbCandidate>>) {
    let mut guard = CANDIDATES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    match list {
        Some(l) => map.insert(game_id.to_string(), l),
        None => map.remove(game_id),
    };
}

fn is_fresh(fetched_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(fetched_at)
        .map(|d| chrono::Utc::now().signed_duration_since(d).num_days() < FRESH_DAYS)
        .unwrap_or(false)
}

fn view(db: &Db, game_id: &str, extra: ViewExtra) -> AppResult<TimeToBeatView> {
    let sources = ttb::list(&db.conn(), game_id)?;
    Ok(TimeToBeatView {
        best: ttb::best(&sources),
        sources: sources.into_iter().filter(|s| !s.is_empty()).collect(),
        candidates: candidates_for(game_id).unwrap_or_default(),
        igdb_keys_missing: extra.keys_missing,
        error: extra.error,
    })
}

#[derive(Default)]
struct ViewExtra {
    keys_missing: bool,
    error: Option<String>,
}

fn igdb_keys(app: &AppHandle) -> AppResult<Option<(String, String)>> {
    let secrets = app.state::<Secrets>();
    Ok(secrets
        .get(SecretKind::IgdbClientId)?
        .zip(secrets.get(SecretKind::IgdbClientSecret)?))
}

/// Fetches IGDB when the cache is missing or old (or `force`). Network errors
/// never fail the command: the cached values come back with `error` set.
fn load(app: &AppHandle, game_id: &str, force: bool) -> AppResult<TimeToBeatView> {
    let db = app.state::<Db>();
    let (game, cached) = {
        let conn = db.conn();
        (
            games::get(&conn, game_id)?,
            ttb::get(&conn, game_id, TtbProvider::Igdb)?,
        )
    };
    if !force
        && (cached.as_ref().is_some_and(|c| is_fresh(&c.fetched_at))
            || candidates_for(game_id).is_some())
    {
        return view(&db, game_id, ViewExtra::default());
    }
    let Some((id, secret)) = igdb_keys(app)? else {
        return view(
            &db,
            game_id,
            ViewExtra {
                keys_missing: true,
                error: None,
            },
        );
    };
    let error = match fetch(&db, &game, &id, &secret) {
        Ok(()) => None,
        Err(e) => {
            tracing::warn!(error = %e, "time to beat: IGDB fetch failed");
            Some(e.to_string())
        }
    };
    view(
        &db,
        game_id,
        ViewExtra {
            keys_missing: false,
            error,
        },
    )
}

fn fetch(db: &Db, game: &games::Game, id: &str, secret: &str) -> AppResult<()> {
    let found = match igdb::find_game(id, secret, game.igdb_id, &game.title)? {
        Match::Found(c) => c,
        Match::Candidates(list) => {
            remember_candidates(&game.id, Some(list));
            return Ok(());
        }
    };
    remember_candidates(&game.id, None);
    let values = igdb::fetch_time_to_beat(id, secret, i64::from(found.id))?;
    let conn = db.conn();
    let igdb_id = i64::from(found.id);
    if game.igdb_id != Some(igdb_id) {
        conn.execute(
            "UPDATE games SET igdb_id = ?2 WHERE id = ?1",
            rusqlite::params![game.id, igdb_id],
        )?;
    }
    // Saved even when IGDB has no values: no new request for 30 days.
    ttb::save_fetched(
        &conn,
        &game.id,
        &TimeToBeat {
            provider: TtbProvider::Igdb,
            external_id: Some(found.id.to_string()),
            main_minutes: values.as_ref().and_then(|v| v.main_minutes),
            extra_minutes: values.as_ref().and_then(|v| v.extra_minutes),
            completionist_minutes: values.as_ref().and_then(|v| v.completionist_minutes),
            sample_count: values.as_ref().and_then(|v| v.count),
            url: found.url,
            fetched_at: now_rfc3339(),
        },
    )
}

async fn run(app: AppHandle, game_id: String, force: bool) -> CommandResult<TimeToBeatView> {
    let result = tauri::async_runtime::spawn_blocking(move || load(&app, &game_id, force))
        .await
        .map_err(|e| AppError::Invalid(format!("time to beat task failed: {e}")))??;
    Ok(result)
}

/// Time to beat of a game (IGDB fetched when missing or older than 30 days).
#[tauri::command]
#[specta::specta]
pub async fn time_to_beat_get(app: AppHandle, game_id: String) -> CommandResult<TimeToBeatView> {
    run(app, game_id, false).await
}

/// "Actualiser": new IGDB lookup, candidates forgotten.
#[tauri::command]
#[specta::specta]
pub async fn time_to_beat_refresh(
    app: AppHandle,
    game_id: String,
) -> CommandResult<TimeToBeatView> {
    remember_candidates(&game_id, None);
    run(app, game_id, true).await
}

/// The user picked the right IGDB game among the candidates.
#[tauri::command]
#[specta::specta]
pub async fn time_to_beat_pick(
    app: AppHandle,
    game_id: String,
    igdb_id: u32,
) -> CommandResult<TimeToBeatView> {
    {
        let db = app.state::<Db>();
        let conn = db.conn();
        games::get(&conn, &game_id)?;
        conn.execute(
            "UPDATE games SET igdb_id = ?2 WHERE id = ?1",
            rusqlite::params![game_id, igdb_id],
        )
        .map_err(AppError::from)?;
    }
    remember_candidates(&game_id, None);
    run(app, game_id, true).await
}

/// The user's correction, in minutes; `None` removes it.
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ManualTimeToBeat {
    pub main: Option<u32>,
    pub extra: Option<u32>,
    pub completionist: Option<u32>,
}

#[tauri::command]
#[specta::specta]
pub fn time_to_beat_set_manual(
    db: tauri::State<'_, Db>,
    game_id: String,
    values: Option<ManualTimeToBeat>,
) -> CommandResult<TimeToBeatView> {
    {
        let conn = db.conn();
        games::get(&conn, &game_id)?;
        ttb::set_manual(
            &conn,
            &game_id,
            values.map(|v| (v.main, v.extra, v.completionist)),
        )?;
    }
    Ok(view(&db, &game_id, ViewExtra::default())?)
}
