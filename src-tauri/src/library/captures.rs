//! Capture scan: finds the captures of every source, ties them to games and
//! keeps the `screenshots` table in step (docs/FICHE-ONGLETS.md). Files are
//! only listed and read.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::db::repo::screenshots::{self, CaptureSource, FoundCapture};
use crate::db::repo::{game_sources, games};
use crate::error::AppResult;
use crate::metadata::thumbs;
use crate::models::SourceKind;
use crate::sources::captures::{self as find, TitleMatcher};

/// Where to look. `None` = that source is absent on this PC.
#[derive(Debug, Clone, Default)]
pub struct ScanPlaces {
    pub steam_root: Option<PathBuf>,
    pub gamebar_dir: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct ScanOutcome {
    /// Folders the webview may read the captures from (`asset:` scope).
    pub dirs: Vec<PathBuf>,
    pub found: usize,
    pub removed: usize,
}

/// How deep to look into a folder added by the user.
const FOLDER_DEPTH: u32 = 3;

fn modified(path: &Path) -> Option<DateTime<Utc>> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()
        .map(DateTime::<Utc>::from)
}

fn size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Time of a capture from its name (Game Bar, then Steam style), else from
/// the file's modification date.
fn taken_from_name(path: &Path) -> Option<DateTime<Utc>> {
    let name = path.file_name()?.to_str()?;
    let stem = path.file_stem()?.to_str()?;
    find::parse_gamebar_name(stem)
        .and_then(|(_, t)| t)
        .or_else(|| find::steam_name_time(name))
        .and_then(find::local_to_utc)
        .or_else(|| modified(path))
}

/// Scans every source and updates the table. Width and height are read from
/// the file header only for files not seen before.
pub fn scan(conn: &Connection, places: &ScanPlaces) -> AppResult<ScanOutcome> {
    let all_games = games::list(conn, true)?;
    let matcher = TitleMatcher::new(all_games.iter().map(|g| (g.id.as_str(), g.title.as_str())));
    let steam_games: HashMap<u64, String> = game_sources::list_by_kind(conn, SourceKind::Steam)?
        .into_iter()
        .filter_map(|s| Some((s.external_id?.parse().ok()?, s.game_id)))
        .collect();
    let known = screenshots::known_paths(conn)?;
    let dims = |path: &Path| {
        if known.contains(&path.display().to_string()) {
            None
        } else {
            thumbs::dimensions(path)
        }
    };

    let mut out = ScanOutcome::default();
    // First source wins when a folder is reachable twice.
    let mut found: HashMap<String, FoundCapture> = HashMap::new();
    let mut add = |c: FoundCapture| {
        found.entry(c.path.clone()).or_insert(c);
    };

    if let Some(root) = &places.steam_root {
        out.dirs.extend(find::steam_remote_dirs(root));
        for shot in find::steam_shots(root) {
            let Some(game_id) = steam_games.get(&shot.appid) else {
                continue;
            };
            let Some(taken) = shot.taken.or_else(|| modified(&shot.path)) else {
                continue;
            };
            let (width, height) = match (shot.width, shot.height) {
                (Some(w), Some(h)) => (Some(w), Some(h)),
                _ => dims(&shot.path).unzip(),
            };
            add(FoundCapture {
                game_id: game_id.clone(),
                source: CaptureSource::Steam,
                path: shot.path.display().to_string(),
                kind: find::kind_of(&shot.path).unwrap_or(screenshots::CaptureKind::Image),
                taken_at: find::fmt_utc(taken),
                width,
                height,
                bytes: size(&shot.path),
                thumb_path: shot.thumb.map(|t| t.display().to_string()),
            });
        }
    }

    let mut folder_jobs: Vec<(PathBuf, Option<String>, CaptureSource)> = Vec::new();
    for f in screenshots::folders(conn)? {
        folder_jobs.push((PathBuf::from(f.path), f.game_id, CaptureSource::Folder));
    }
    if let Some(dir) = &places.gamebar_dir {
        folder_jobs.push((dir.clone(), None, CaptureSource::Gamebar));
    }
    // Game Bar first: its files keep their own badge inside a user folder.
    folder_jobs.sort_by_key(|(_, _, s)| *s != CaptureSource::Gamebar);

    for (dir, game, source) in folder_jobs {
        if !dir.is_dir() {
            continue;
        }
        out.dirs.push(dir.clone());
        let depth = if source == CaptureSource::Gamebar {
            0
        } else {
            FOLDER_DEPTH
        };
        for path in find::media_files(&dir, depth) {
            let game_id = match &game {
                Some(id) => id.as_str(),
                None => match find::match_file(&matcher, &dir, &path) {
                    Some(id) => id,
                    None => continue,
                },
            };
            let Some(taken) = taken_from_name(&path) else {
                continue;
            };
            let kind = find::kind_of(&path).unwrap_or(screenshots::CaptureKind::Image);
            let (width, height) = if kind == screenshots::CaptureKind::Image {
                dims(&path).unzip()
            } else {
                (None, None)
            };
            add(FoundCapture {
                game_id: game_id.to_string(),
                source,
                path: path.display().to_string(),
                kind,
                taken_at: find::fmt_utc(taken),
                width,
                height,
                bytes: size(&path),
                thumb_path: None,
            });
        }
    }

    let seen: HashSet<String> = found.keys().cloned().collect();
    let tx = conn.unchecked_transaction()?;
    for c in found.values() {
        screenshots::upsert(&tx, c)?;
    }
    let mut removed = 0;
    let stale = screenshots::remove_where(&tx, |c| {
        let gone = !seen.contains(&c.path);
        removed += usize::from(gone);
        gone
    })?;
    tx.commit()?;
    out.found = seen.len();
    out.removed = removed;
    drop_generated_thumbs(&stale);
    Ok(out)
}

/// Deletes thumbnails we made (never a Steam one) for captures now gone.
fn drop_generated_thumbs(paths: &[String]) {
    for p in paths {
        let path = Path::new(p);
        let ours = path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "thumbs")
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|n| n == "artwork");
        if ours {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Makes the thumbnails still missing; the DB is locked only to read the
/// list and to save each result. Returns how many were made.
pub fn make_thumbs(db: &crate::db::Db, data_dir: &Path) -> usize {
    let todo = match screenshots::missing_thumbs(&db.conn()) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "captures: cannot list missing thumbnails");
            return 0;
        }
    };
    let dir = thumbs::dir(data_dir);
    let mut made = 0;
    for (id, path) in todo {
        let source = Path::new(&path);
        let dest = thumbs::target(&dir, source);
        if !dest.is_file() {
            if let Err(e) = thumbs::make(source, &dest) {
                tracing::debug!(%path, error = %e, "captures: thumbnail failed");
                continue;
            }
        }
        if screenshots::set_thumb(&db.conn(), &id, &dest.display().to_string()).is_ok() {
            made += 1;
        }
    }
    made
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::Db;

    #[test]
    fn scans_steam_game_bar_and_user_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let steam = tmp.path().join("Steam");
        let shots = steam.join(r"userdata\1\760\remote\392110\screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        std::fs::write(shots.join("20251029112231_1.jpg"), b"x").unwrap();
        std::fs::write(
            steam.join(r"userdata\1\760\remote\999\screenshots.txt"),
            b"",
        )
        .ok();
        let gamebar = tmp.path().join("Captures");
        std::fs::create_dir_all(&gamebar).unwrap();
        std::fs::write(gamebar.join("Hades 2025-10-30 21-05-00.png"), b"x").unwrap();
        std::fs::write(gamebar.join("Bureau 2025-10-30 21-05-00.png"), b"x").unwrap();
        let mine = tmp.path().join("Mine");
        std::fs::create_dir_all(mine.join("sub")).unwrap();
        std::fs::write(mine.join(r"sub\clip.mp4"), b"xx").unwrap();

        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let es2 = games::insert(&conn, "Endless Space 2").unwrap();
        let hades = games::insert(&conn, "Hades").unwrap();
        game_sources::upsert(
            &conn,
            &es2.id,
            SourceKind::Steam,
            &SourceData {
                external_id: Some("392110".into()),
                ..Default::default()
            },
        )
        .unwrap();
        screenshots::add_folder(&conn, &mine.display().to_string(), Some(&hades.id)).unwrap();

        let places = ScanPlaces {
            steam_root: Some(steam.clone()),
            gamebar_dir: Some(gamebar.clone()),
        };
        let out = scan(&conn, &places).unwrap();
        assert_eq!(out.found, 3);
        let es2_shots = screenshots::list_for_game(&conn, &es2.id).unwrap();
        assert_eq!(es2_shots[0].source, CaptureSource::Steam);
        let hades_shots = screenshots::list_for_game(&conn, &hades.id).unwrap();
        let sources: Vec<_> = hades_shots.iter().map(|c| c.source).collect();
        assert!(sources.contains(&CaptureSource::Gamebar));
        assert!(sources.contains(&CaptureSource::Folder));
        let gb = hades_shots
            .iter()
            .find(|c| c.source == CaptureSource::Gamebar)
            .unwrap();
        assert_eq!(
            gb.taken_at,
            find::fmt_utc(
                find::local_to_utc(
                    chrono::NaiveDateTime::parse_from_str(
                        "2025-10-30 21:05:00",
                        "%Y-%m-%d %H:%M:%S"
                    )
                    .unwrap()
                )
                .unwrap()
            )
        );

        // A removed folder takes its captures with it; files are untouched.
        screenshots::remove_folder(&conn, &mine.display().to_string()).unwrap();
        let out = scan(&conn, &places).unwrap();
        assert_eq!((out.found, out.removed), (2, 1));
        assert!(mine.join(r"sub\clip.mp4").is_file());
    }
}
