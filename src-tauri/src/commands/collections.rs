//! Collections (manual and smart) and search syntax helpers.

use serde::Serialize;

use super::CommandResult;
use crate::db::repo::collections::{self, Collection, CollectionKind};
use crate::db::Db;
use crate::library::query::{self, Context, LibraryQuery, LibraryView, SortKey};
use crate::library::rules::Rule;
use crate::library::search;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionInfo {
    pub collection: Collection,
    /// Visible games currently in the collection.
    pub count: u32,
}

#[tauri::command]
#[specta::specta]
pub fn collections_list(db: tauri::State<'_, Db>) -> CommandResult<Vec<CollectionInfo>> {
    let conn = db.conn();
    let all = query::summaries(&conn)?;
    let ctx = Context::load(&conn, &all)?;
    let list = collections::list(&conn)?;
    drop(conn);
    Ok(list
        .into_iter()
        .map(|c| {
            let q = LibraryQuery {
                view: LibraryView::Collection(c.id.clone()),
                text: None,
                sort: SortKey::Name,
            };
            let count = query::filter_and_sort(all.clone(), &q, &ctx).len() as u32;
            CollectionInfo {
                collection: c,
                count,
            }
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn collection_create(
    db: tauri::State<'_, Db>,
    name: String,
    kind: CollectionKind,
    rules: Option<String>,
) -> CommandResult<Collection> {
    Ok(collections::create(
        &db.conn(),
        &name,
        kind,
        rules.as_deref(),
    )?)
}

#[tauri::command]
#[specta::specta]
pub fn collection_update(
    db: tauri::State<'_, Db>,
    id: String,
    name: String,
    rules: Option<String>,
) -> CommandResult<Collection> {
    Ok(collections::update(
        &db.conn(),
        &id,
        &name,
        rules.as_deref(),
    )?)
}

#[tauri::command]
#[specta::specta]
pub fn collection_delete(db: tauri::State<'_, Db>, id: String) -> CommandResult<()> {
    Ok(collections::delete(&db.conn(), &id)?)
}

#[tauri::command]
#[specta::specta]
pub fn collection_set_member(
    db: tauri::State<'_, Db>,
    id: String,
    game_id: String,
    member: bool,
) -> CommandResult<()> {
    Ok(collections::set_member(&db.conn(), &id, &game_id, member)?)
}

/// Number of visible games matching rules being edited (live preview).
#[tauri::command]
#[specta::specta]
pub fn rules_preview(db: tauri::State<'_, Db>, rules: String) -> CommandResult<u32> {
    let rule = Rule::parse(&rules)?;
    let conn = db.conn();
    let all = query::summaries(&conn)?;
    let ctx = Context::load(&conn, &all)?;
    let now = chrono::Utc::now();
    Ok(all
        .iter()
        .filter(|g| !g.hidden)
        .filter(|g| ctx.facts.get(&g.id).is_some_and(|f| rule.matches(f, now)))
        .count() as u32)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchCheck {
    /// Tokens that looked like filters but were not understood.
    pub warnings: Vec<String>,
    /// The search as rules JSON, to save it as a smart collection.
    pub rules: String,
}

#[tauri::command]
#[specta::specta]
pub fn search_check(text: String) -> CommandResult<SearchCheck> {
    let parsed = search::parse(&text);
    Ok(SearchCheck {
        warnings: parsed.warnings,
        rules: serde_json::to_string(&parsed.rule).map_err(crate::error::AppError::from)?,
    })
}

/// Ids of the manual collections containing a game.
#[tauri::command]
#[specta::specta]
pub fn game_collections(db: tauri::State<'_, Db>, game_id: String) -> CommandResult<Vec<String>> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare("SELECT collection_id FROM collection_games WHERE game_id = ?1")
        .map_err(crate::error::AppError::from)?;
    let ids = stmt
        .query_map([game_id], |r| r.get(0))
        .and_then(|rows| rows.collect::<Result<Vec<String>, _>>())
        .map_err(crate::error::AppError::from)?;
    Ok(ids)
}

/// Statistics for a period, optionally limited to some sources.
#[tauri::command]
#[specta::specta]
pub fn stats_overview(
    db: tauri::State<'_, Db>,
    range: crate::library::stats::StatsRange,
    sources: Vec<crate::models::SourceKind>,
) -> CommandResult<crate::library::stats::StatsOverview> {
    let today = chrono::Local::now().date_naive();
    Ok(crate::library::stats::overview(
        &db.conn(),
        range,
        &sources,
        today,
    )?)
}
