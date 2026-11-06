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
    let conn = db.conn();
    let all = query::summaries(&conn)?;
    let ctx = query::Context::load(&conn, &all)?;
    Ok(query::filter_and_sort(all, &query, &ctx))
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
    pub genres: Vec<String>,
    /// Player modes (Solo, Multijoueur, Coopération, Écran partagé).
    pub modes: Vec<String>,
    /// Source the Play button uses (same rule as `launch_game`).
    pub play_source_id: Option<String>,
    pub sources: Vec<SourceInfo>,
    pub time_by_source: Vec<SourceTime>,
}

fn to_u32(v: i64) -> u32 {
    v.clamp(0, u32::MAX as i64) as u32
}

pub(crate) fn load_detail(db: &Db, id: &str) -> CommandResult<GameDetail> {
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
    let play_source_id =
        crate::launcher::pick_source(&sources, game.preferred_source_id.as_deref())
            .map(|s| s.id.clone());
    Ok(GameDetail {
        play_source_id,
        notes: game.notes,
        preferred_source_id: game.preferred_source_id,
        hero: art.remove("hero"),
        logo: art.remove("logo"),
        tags: tags::for_game(&conn, id)?,
        genres: auto_tags(&conn, id, "genre")?,
        modes: auto_tags(&conn, id, "mode")?,
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
    /// Source used by the Play button; an empty string resets to automatic.
    pub preferred_source_id: Option<String>,
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
        preferred_source_id: update
            .preferred_source_id
            .map(|s| (!s.is_empty()).then_some(s)),
        ..Default::default()
    };
    if let Some(Some(source_id)) = &patch.preferred_source_id {
        let source = game_sources::get(&db.conn(), source_id)?;
        if source.game_id != id {
            return Err(AppError::Invalid("source belongs to another game".into()).into());
        }
    }
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
pub fn launch_game(
    db: tauri::State<'_, Db>,
    tracker: tauri::State<'_, crate::tracker::TrackerState>,
    game_id: String,
) -> CommandResult<()> {
    tracker.set_launch_hint(&game_id);
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
    let args = crate::sources::local::default_args(&db.conn(), &source.id)?;
    crate::launcher::launch(source, args.as_deref()).map_err(CommandError::from)
}

/// Groups of games that look like the same title on several platforms.
#[tauri::command]
#[specta::specta]
pub fn duplicates_list(db: tauri::State<'_, Db>) -> CommandResult<Vec<Vec<GameSummary>>> {
    let conn = db.conn();
    let groups = crate::library::merge::duplicate_groups(&conn)?;
    let all = query::summaries(&conn)?;
    Ok(groups
        .into_iter()
        .map(|ids| {
            all.iter()
                .filter(|g| ids.contains(&g.id))
                .cloned()
                .collect()
        })
        .collect())
}

/// Merges `others` into `target`: one sheet, several sources.
#[tauri::command]
#[specta::specta]
pub fn game_merge(
    db: tauri::State<'_, Db>,
    target_id: String,
    other_ids: Vec<String>,
) -> CommandResult<GameDetail> {
    crate::library::merge::merge(&mut db.conn(), &target_id, &other_ids)?;
    load_detail(&db, &target_id)
}

/// Detaches a source into its own game. Returns the new game id.
#[tauri::command]
#[specta::specta]
pub fn game_split(db: tauri::State<'_, Db>, source_id: String) -> CommandResult<String> {
    Ok(crate::library::merge::split(&mut db.conn(), &source_id)?)
}

/// Reads a picked `.exe` / `.lnk` / `.url` to prefill the "add a game" form.
#[tauri::command]
#[specta::specta]
pub fn exe_inspect(path: String) -> CommandResult<crate::sources::local::ExeInfo> {
    Ok(crate::sources::local::inspect(std::path::Path::new(&path))?)
}

/// Adds a game from an executable or a launcher link.
#[tauri::command]
#[specta::specta]
pub fn add_executable(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    input: crate::sources::local::AddExeInput,
) -> CommandResult<GameDetail> {
    let id = crate::sources::local::create(&mut db.conn(), &input)?;
    super::artwork::spawn_autofill(app);
    load_detail(&db, &id)
}

fn auto_tags(
    conn: &rusqlite::Connection,
    game_id: &str,
    origin: &str,
) -> crate::error::AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN game_tags gt ON gt.tag_id = t.id \
         WHERE gt.game_id = ?1 AND t.origin = ?2 ORDER BY t.name",
    )?;
    let prefix = format!("{origin}:");
    let rows = stmt.query_map(rusqlite::params![game_id, origin], |r| {
        r.get::<_, String>(0)
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?.trim_start_matches(&prefix).to_string());
    }
    Ok(out)
}

#[derive(Debug, Serialize, specta::Type)]
pub struct Facet {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Serialize, specta::Type)]
pub struct Facets {
    pub genres: Vec<Facet>,
    pub modes: Vec<Facet>,
}

/// Genres and player modes present in the library, for the filter chips.
#[tauri::command]
#[specta::specta]
pub fn facets(db: tauri::State<'_, Db>) -> CommandResult<Facets> {
    let conn = db.conn();
    let to = |v: Vec<(String, u32)>| {
        v.into_iter()
            .map(|(name, count)| Facet { name, count })
            .collect()
    };
    Ok(Facets {
        genres: to(tags::auto_facets(&conn, "genre")?),
        modes: to(tags::auto_facets(&conn, "mode")?),
    })
}

/// Looks for games in the sub-folders of the given directories.
#[tauri::command]
#[specta::specta]
pub async fn folders_scan(
    db: tauri::State<'_, Db>,
    paths: Vec<String>,
) -> CommandResult<Vec<crate::sources::folder_scan::FolderCandidate>> {
    let known: Vec<String> = game_sources::list_all(&db.conn())?
        .into_iter()
        .flat_map(|s| [s.install_path, s.exe_path])
        .flatten()
        .collect();
    let roots: Vec<std::path::PathBuf> = paths.iter().map(std::path::PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || crate::sources::folder_scan::scan(&roots, &known))
        .await
        .map_err(|e| AppError::Invalid(format!("scan task failed: {e}")).into())
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScannedGame {
    pub title: String,
    pub exe_path: String,
}

/// Adds several scanned games at once (automatic process tracking).
/// Returns how many were added; failures (duplicates...) are skipped.
#[tauri::command]
#[specta::specta]
pub fn add_scanned_games(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    games: Vec<ScannedGame>,
) -> CommandResult<u32> {
    let mut added = 0;
    for g in games {
        let input = crate::sources::local::AddExeInput {
            title: g.title,
            exe_path: Some(g.exe_path),
            uri: None,
            args: None,
            track: crate::sources::local::TrackMode::Auto,
            estimate_minutes: None,
        };
        match crate::sources::local::create(&mut db.conn(), &input) {
            Ok(_) => added += 1,
            Err(e) => tracing::info!(error = %e, "scanned game skipped"),
        }
    }
    if added > 0 {
        super::artwork::spawn_autofill(app);
    }
    Ok(added)
}

/// Changes applied to several games at once (multi-selection).
#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BulkUpdate {
    pub status: Option<GameStatus>,
    pub hidden: Option<bool>,
    pub favorite: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub fn games_bulk_update(
    db: tauri::State<'_, Db>,
    ids: Vec<String>,
    update: BulkUpdate,
) -> CommandResult<()> {
    let patch = games::GamePatch {
        status: update.status,
        hidden: update.hidden,
        favorite: update.favorite,
        ..Default::default()
    };
    let mut conn = db.conn();
    let tx = conn.transaction().map_err(AppError::from)?;
    for id in &ids {
        games::update(&tx, id, &patch)?;
    }
    tx.commit().map_err(AppError::from)?;
    Ok(())
}

/// Only games made of executables added by hand can be deleted: the others
/// would come back at the next sync of their launcher (hide them instead).
pub fn deletable(kinds: &[SourceKind]) -> bool {
    !kinds.is_empty() && kinds.iter().all(|k| *k == SourceKind::Local)
}

/// Deletes executable games with their sessions, goals and cached images.
/// The executables themselves are left untouched.
#[tauri::command]
#[specta::specta]
pub fn games_delete(
    db: tauri::State<'_, Db>,
    paths: tauri::State<'_, super::app::AppPaths>,
    ids: Vec<String>,
) -> CommandResult<()> {
    let mut conn = db.conn();
    let tx = conn.transaction().map_err(AppError::from)?;
    for id in &ids {
        let kinds: Vec<SourceKind> = game_sources::list_for_game(&tx, id)?
            .into_iter()
            .map(|s| s.kind)
            .collect();
        if !deletable(&kinds) {
            return Err(
                AppError::Invalid(format!("game {id} is not made of executables only")).into(),
            );
        }
        games::delete(&tx, id)?;
    }
    tx.commit().map_err(AppError::from)?;
    let root = crate::metadata::artwork::cache_root(&paths.data_dir);
    for id in &ids {
        // Ids are UUIDs: never a path outside the cache.
        if uuid::Uuid::parse_str(id).is_ok() {
            let _ = std::fs::remove_dir_all(root.join(id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod bulk_tests {
    use super::*;

    #[test]
    fn only_executable_games_can_be_deleted() {
        assert!(deletable(&[SourceKind::Local]));
        assert!(deletable(&[SourceKind::Local, SourceKind::Local]));
        assert!(!deletable(&[SourceKind::Local, SourceKind::Steam]));
        assert!(!deletable(&[SourceKind::Gog]));
        assert!(!deletable(&[]));
    }
}
