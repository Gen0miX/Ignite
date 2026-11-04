use serde::{Deserialize, Serialize};

use super::{CommandError, CommandResult};
use crate::db::repo::{artwork, game_sources, games, tags};
use crate::db::Db;
use crate::error::AppError;
use crate::library::playtime;
use crate::library::query::{self, GameSummary, LibraryCounts, LibraryQuery};
use crate::models::{GameStatus, SourceKind};

#[tauri::command]
#[specta::specta]
pub fn library_list(
    db: tauri::State<'_, Db>,
    query: LibraryQuery,
) -> CommandResult<Vec<GameSummary>> {
    let all = query::summaries(&db.conn())?;
    Ok(query::filter_and_sort(all, &query))
}

#[tauri::command]
#[specta::specta]
pub fn library_counts(db: tauri::State<'_, Db>) -> CommandResult<LibraryCounts> {
    Ok(query::counts(&query::summaries(&db.conn())?))
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub id: String,
    pub kind: SourceKind,
    pub external_id: Option<String>,
    pub installed: bool,
    pub install_path: Option<String>,
    pub size_bytes: Option<f64>,
    pub launch_uri: Option<String>,
    pub playtime_minutes: Option<u32>,
    pub last_played: Option<String>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceTime {
    pub source_id: Option<String>,
    pub kind: Option<SourceKind>,
    pub minutes: u32,
    pub from_platform: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub summary: GameSummary,
    pub notes: Option<String>,
    pub preferred_source_id: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub tags: Vec<tags::Tag>,
    pub sources: Vec<SourceInfo>,
    pub time_by_source: Vec<SourceTime>,
}

fn to_u32(v: i64) -> u32 {
    v.clamp(0, u32::MAX as i64) as u32
}

fn load_detail(db: &Db, id: &str) -> CommandResult<GameDetail> {
    let conn = db.conn();
    let game = games::get(&conn, id)?;
    let summary = query::summaries(&conn)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| AppError::NotFound(format!("game {id}")))?;
    let sources = game_sources::list_for_game(&conn, id)?;
    let session_sums: Vec<_> = crate::db::repo::sessions::sums(&conn)?
        .into_iter()
        .filter(|s| s.game_id == id)
        .map(|s| (s.game_source_id, s.seconds))
        .collect();
    let parts = playtime::breakdown(
        &sources
            .iter()
            .map(|s| (s.id.clone(), s.source_playtime_minutes))
            .collect::<Vec<_>>(),
        &session_sums,
    );
    let mut art = artwork::for_game(&conn, id)?;
    Ok(GameDetail {
        notes: game.notes,
        preferred_source_id: game.preferred_source_id,
        hero: art.remove("hero"),
        logo: art.remove("logo"),
        tags: tags::for_game(&conn, id)?,
        time_by_source: parts
            .into_iter()
            .map(|p| SourceTime {
                kind: p
                    .source_id
                    .as_ref()
                    .and_then(|sid| sources.iter().find(|s| &s.id == sid))
                    .map(|s| s.kind),
                source_id: p.source_id,
                minutes: to_u32(p.minutes),
                from_platform: p.from_platform,
            })
            .collect(),
        sources: sources
            .into_iter()
            .map(|s| SourceInfo {
                id: s.id,
                kind: s.kind,
                external_id: s.external_id,
                installed: s.installed,
                install_path: s.install_path,
                size_bytes: s.size_bytes.map(|b| b as f64),
                launch_uri: s.launch_uri,
                playtime_minutes: s.source_playtime_minutes.map(to_u32),
                last_played: s.source_last_played,
            })
            .collect(),
        summary,
    })
}

#[tauri::command]
#[specta::specta]
pub fn game_get(db: tauri::State<'_, Db>, id: String) -> CommandResult<GameDetail> {
    load_detail(&db, &id)
}

/// Fields editable from the game panel. An empty `notes` string clears the note.
#[derive(Debug, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameUpdate {
    pub status: Option<GameStatus>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    pub notes: Option<String>,
}

const MAX_NOTES_LEN: usize = 10_000;

#[tauri::command]
#[specta::specta]
pub fn game_update(
    db: tauri::State<'_, Db>,
    id: String,
    update: GameUpdate,
) -> CommandResult<GameDetail> {
    if update
        .notes
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_NOTES_LEN)
    {
        return Err(AppError::Invalid("note is too long".into()).into());
    }
    let patch = games::GamePatch {
        status: update.status,
        favorite: update.favorite,
        hidden: update.hidden,
        notes: update.notes.map(|n| {
            let n = n.trim().to_string();
            (!n.is_empty()).then_some(n)
        }),
        ..Default::default()
    };
    games::update(&db.conn(), &id, &patch)?;
    load_detail(&db, &id)
}

#[tauri::command]
#[specta::specta]
pub fn game_tag_add(
    db: tauri::State<'_, Db>,
    id: String,
    name: String,
) -> CommandResult<tags::Tag> {
    let conn = db.conn();
    games::get(&conn, &id)?;
    Ok(tags::add_to_game(&conn, &id, &name)?)
}

#[tauri::command]
#[specta::specta]
pub fn game_tag_remove(db: tauri::State<'_, Db>, id: String, tag_id: String) -> CommandResult<()> {
    Ok(tags::remove_from_game(&db.conn(), &id, &tag_id)?)
}

#[tauri::command]
#[specta::specta]
pub fn launch_game(db: tauri::State<'_, Db>, game_id: String) -> CommandResult<()> {
    let (sources, preferred) = {
        let conn = db.conn();
        let game = games::get(&conn, &game_id)?;
        (
            game_sources::list_for_game(&conn, &game_id)?,
            game.preferred_source_id,
        )
    };
    let source = crate::launcher::pick_source(&sources, preferred.as_deref())
        .ok_or_else(|| AppError::Invalid("no launchable source for this game".into()))?;
    if source.kind == SourceKind::Steam && !super::sources::steam_installed() {
        return Err(AppError::Invalid("Steam is not installed on this PC".into()).into());
    }
    crate::launcher::launch(source).map_err(CommandError::from)
}
