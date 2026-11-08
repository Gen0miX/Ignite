//! Captures tab and viewer (docs/FICHE-ONGLETS.md). Captures are scanned in
//! the background; the files themselves are only read (shown, copied to the
//! clipboard, copied into the image cache as a background).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::app::AppPaths;
use super::CommandResult;
use crate::db::repo::screenshots::{self, Capture, CaptureFolder, CaptureKind};
use crate::db::repo::{games, sessions};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::library::captures::{self as scan, ScanPlaces};
use crate::metadata::artwork;
use crate::sources::captures as find;
use crate::sources::steam::local::locate_steam;

/// Emitted after a scan or once thumbnails are made: the UI reloads.
pub const CAPTURES_CHANGED_EVENT: &str = "captures-changed";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSession {
    pub id: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_seconds: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureItem {
    pub capture: Capture,
    /// Session the capture was taken in (its time is within the session).
    pub session: Option<CaptureSession>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CapturesView {
    pub items: Vec<CaptureItem>,
    /// Folders added for this game or for all games.
    pub folders: Vec<CaptureFolder>,
    pub scanning: bool,
}

fn parse(t: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(t)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

pub(crate) fn items_for_game(
    conn: &rusqlite::Connection,
    game_id: &str,
) -> AppResult<Vec<CaptureItem>> {
    let list = sessions::list_for_game(conn, game_id)?;
    let spans: Vec<(String, DateTime<Utc>, DateTime<Utc>)> = list
        .iter()
        .filter_map(|s| {
            Some((
                s.id.clone(),
                parse(s.started_at.as_deref()?)?,
                parse(s.ended_at.as_deref()?)?,
            ))
        })
        .collect();
    let items = screenshots::list_for_game(conn, game_id)?
        .into_iter()
        .map(|capture| {
            let session = parse(&capture.taken_at)
                .and_then(|t| find::session_for(t, &spans))
                .and_then(|id| list.iter().find(|s| s.id == id))
                .map(|s| CaptureSession {
                    id: s.id.clone(),
                    started_at: s.started_at.clone().unwrap_or_default(),
                    ended_at: s.ended_at.clone().unwrap_or_default(),
                    duration_seconds: s.duration_seconds,
                });
            CaptureItem { capture, session }
        })
        .collect();
    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub fn captures_list(db: tauri::State<'_, Db>, game_id: String) -> CommandResult<CapturesView> {
    let conn = db.conn();
    let items = items_for_game(&conn, &game_id)?;
    let folders = screenshots::folders(&conn)?
        .into_iter()
        .filter(|f| f.game_id.as_deref().is_none_or(|g| g == game_id))
        .collect();
    Ok(CapturesView {
        items,
        folders,
        scanning: SCANNING.load(Ordering::SeqCst),
    })
}

static SCANNING: AtomicBool = AtomicBool::new(false);
static AGAIN: AtomicBool = AtomicBool::new(false);

fn places() -> ScanPlaces {
    ScanPlaces {
        steam_root: locate_steam(),
        gamebar_dir: find::gamebar_dir(),
    }
}

fn allow_dirs(app: &AppHandle, dirs: &[PathBuf]) {
    let scope = app.asset_protocol_scope();
    for d in dirs {
        if let Err(e) = scope.allow_directory(d, true) {
            tracing::warn!(dir = %d.display(), error = %e, "captures: cannot serve folder");
        }
    }
}

/// Scans now, on this thread (end of session: the recap shows the captures).
/// Thumbnails are made afterwards in the background.
pub fn scan_now(app: &AppHandle) {
    if SCANNING.swap(true, Ordering::SeqCst) {
        AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    loop {
        AGAIN.store(false, Ordering::SeqCst);
        let outcome = {
            let db = app.state::<Db>();
            let conn = db.conn();
            scan::scan(&conn, &places())
        };
        match outcome {
            Ok(out) => {
                allow_dirs(app, &out.dirs);
                tracing::debug!(found = out.found, removed = out.removed, "captures scanned");
            }
            Err(e) => tracing::warn!(error = %e, "captures scan failed"),
        }
        if !AGAIN.load(Ordering::SeqCst) {
            break;
        }
    }
    SCANNING.store(false, Ordering::SeqCst);
    let _ = app.emit(CAPTURES_CHANGED_EVENT, ());
    let app = app.clone();
    std::thread::spawn(move || {
        let data_dir = app.state::<AppPaths>().data_dir.clone();
        if scan::make_thumbs(&app.state::<Db>(), &data_dir) > 0 {
            let _ = app.emit(CAPTURES_CHANGED_EVENT, ());
        }
    });
}

/// Scans in the background (startup, opening the tab, folder changes).
pub fn scan_later(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || scan_now(&app));
}

#[tauri::command]
#[specta::specta]
pub fn captures_scan(app: AppHandle) {
    scan_later(&app);
}

#[tauri::command]
#[specta::specta]
pub fn capture_set_caption(
    db: tauri::State<'_, Db>,
    id: String,
    caption: Option<String>,
) -> CommandResult<()> {
    Ok(screenshots::set_caption(
        &db.conn(),
        &id,
        caption.as_deref(),
    )?)
}

fn capture(app: &AppHandle, id: &str) -> AppResult<Capture> {
    let db = app.state::<Db>();
    let c = screenshots::get(&db.conn(), id)?;
    if !Path::new(&c.path).is_file() {
        return Err(AppError::NotFound(format!("capture file {}", c.path)));
    }
    Ok(c)
}

/// "Utiliser comme fond du jeu": a copy goes into the image cache as the
/// game's hero; the capture itself is left alone.
#[tauri::command]
#[specta::specta]
pub async fn capture_use_as_hero(app: AppHandle, id: String) -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let c = capture(&app, &id)?;
        if c.kind != CaptureKind::Image {
            return Err(AppError::Invalid("a video cannot be a background".into()));
        }
        let data_dir = app.state::<AppPaths>().data_dir.clone();
        let db = app.state::<Db>();
        let conn = db.conn();
        games::get(&conn, &c.game_id)?;
        artwork::set_from_file(&conn, &data_dir, &c.game_id, "hero", Path::new(&c.path))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Invalid(format!("capture task failed: {e}")))??;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn capture_copy(app: AppHandle, id: String) -> CommandResult<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    tauri::async_runtime::spawn_blocking(move || {
        let c = capture(&app, &id)?;
        if c.kind != CaptureKind::Image {
            return Err(AppError::Invalid("only images can be copied".into()));
        }
        let img = image::open(&c.path)
            .map_err(|e| AppError::Invalid(format!("cannot read image: {e}")))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        app.clipboard()
            .write_image(&tauri::image::Image::new_owned(img.into_raw(), w, h))
            .map_err(|e| AppError::Invalid(format!("clipboard: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Invalid(format!("capture task failed: {e}")))??;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn capture_reveal(app: AppHandle, id: String) -> CommandResult<()> {
    let c = capture(&app, &id)?;
    tauri_plugin_opener::reveal_item_in_dir(&c.path)
        .map_err(|e| AppError::Invalid(format!("cannot open Explorer: {e}")))?;
    Ok(())
}

/// Opens the capture with the default app of Windows.
#[tauri::command]
#[specta::specta]
pub fn capture_open(app: AppHandle, id: String) -> CommandResult<()> {
    let c = capture(&app, &id)?;
    tauri_plugin_opener::open_path(&c.path, None::<&str>)
        .map_err(|e| AppError::Invalid(format!("cannot open capture: {e}")))?;
    Ok(())
}

/// "Ouvrir le dossier": the folder holding this capture.
#[tauri::command]
#[specta::specta]
pub fn capture_open_folder(app: AppHandle, id: String) -> CommandResult<()> {
    let c = capture(&app, &id)?;
    let dir = Path::new(&c.path)
        .parent()
        .ok_or_else(|| AppError::Invalid("capture has no folder".into()))?;
    tauri_plugin_opener::open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| AppError::Invalid(format!("cannot open folder: {e}")))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn capture_folders_list(db: tauri::State<'_, Db>) -> CommandResult<Vec<CaptureFolder>> {
    Ok(screenshots::folders(&db.conn())?)
}

/// Adds a folder for one game, or for all games (`game_id` = `None`).
#[tauri::command]
#[specta::specta]
pub fn capture_folder_add(
    app: AppHandle,
    path: String,
    game_id: Option<String>,
) -> CommandResult<()> {
    if !Path::new(&path).is_dir() {
        return Err(AppError::Invalid(format!("not a folder: {path}")).into());
    }
    {
        let db = app.state::<Db>();
        let conn = db.conn();
        if let Some(id) = &game_id {
            games::get(&conn, id)?;
        }
        screenshots::add_folder(&conn, &path, game_id.as_deref())?;
    }
    scan_later(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn capture_folder_remove(app: AppHandle, path: String) -> CommandResult<()> {
    screenshots::remove_folder(&app.state::<Db>().conn(), &path)?;
    scan_later(&app);
    Ok(())
}
