//! Artwork cache: every image is copied into `<app-data>/artwork/<game_id>/`
//! and served to the frontend through the `asset:` protocol (no hotlinking).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::db::repo::artwork::{self, ArtworkRow};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::http;
use crate::library::sync::ArtworkJob;
use crate::sources::{ArtworkKind, ArtworkSource};

const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;
/// Do not retry a URL that failed less than this long ago.
const MISS_RETRY_DAYS: i64 = 7;

pub fn cache_root(data_dir: &Path) -> PathBuf {
    data_dir.join("artwork")
}

/// Detects the image format from its first bytes; `None` = not an image.
pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        _ => None,
    }
}

fn write_image(dir: &Path, kind: ArtworkKind, bytes: &[u8]) -> AppResult<PathBuf> {
    let ext = image_extension(bytes).ok_or_else(|| AppError::Invalid("not an image".into()))?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.{ext}", kind.as_str()));
    let tmp = dir.join(format!("{}.{ext}.part", kind.as_str()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Downloads an image to `<dir>/<name>.<ext>` (extension from its content).
pub fn download_named(dir: &Path, name: &str, url: &str) -> AppResult<PathBuf> {
    let bytes = download(url)?;
    let ext = image_extension(&bytes).ok_or_else(|| AppError::Invalid("not an image".into()))?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{name}.{ext}"));
    let tmp = dir.join(format!("{name}.{ext}.part"));
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn download(url: &str) -> AppResult<Vec<u8>> {
    let mut response = http::agent()
        .get(url)
        .call()
        .map_err(|e| http::network_error(e, None))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_IMAGE_BYTES)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// URLs that recently failed (404 for old games without a 600x900 cover),
/// persisted in `artwork/misses.json` so launches stay quiet.
pub struct MissCache {
    path: PathBuf,
    entries: Mutex<HashMap<String, String>>,
}

impl MissCache {
    pub fn load(data_dir: &Path) -> Self {
        let path = cache_root(data_dir).join("misses.json");
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    pub fn is_recent_miss(&self, url: &str) -> bool {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .get(url)
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
            .is_some_and(|d| {
                chrono::Utc::now().signed_duration_since(d).num_days() < MISS_RETRY_DAYS
            })
    }

    pub fn record(&self, url: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(url.to_string(), crate::db::now_rfc3339());
    }

    pub fn save(&self) {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec(&*entries) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

/// Fills missing artwork. With `network = false` only local files are copied
/// (fast, done during the sync); with `true` CDN URLs are downloaded too.
/// Returns the number of images stored. Failures are logged, never fatal.
pub fn fill(
    db: &Db,
    data_dir: &Path,
    jobs: &[ArtworkJob],
    network: bool,
    misses: &MissCache,
) -> usize {
    let mut stored = 0;
    for job in jobs {
        let dir = cache_root(data_dir).join(&job.game_id);
        let have: HashSet<String> = match artwork::for_game(&db.conn(), &job.game_id) {
            Ok(map) => map
                .into_iter()
                .filter(|(_, p)| Path::new(p).is_file())
                .map(|(k, _)| k)
                .collect(),
            Err(_) => continue,
        };
        for candidate in &job.candidates {
            if have.contains(candidate.kind.as_str()) {
                continue;
            }
            for source in &candidate.sources {
                let (bytes, url) = match source {
                    ArtworkSource::File(path) => {
                        (std::fs::read(path).map_err(AppError::from), None)
                    }
                    ArtworkSource::Url(url) if network && !misses.is_recent_miss(url) => {
                        (download(url), Some(url.clone()))
                    }
                    ArtworkSource::Url(_) => continue,
                };
                let saved = bytes.and_then(|b| write_image(&dir, candidate.kind, &b));
                match saved {
                    Ok(path) => {
                        let row = ArtworkRow {
                            game_id: job.game_id.clone(),
                            kind: candidate.kind.as_str().to_string(),
                            local_path: path.display().to_string(),
                            provider: Some("steam".into()),
                            source_url: url,
                        };
                        if artwork::upsert(&db.conn(), &row).is_ok() {
                            stored += 1;
                        }
                        break;
                    }
                    Err(e) => {
                        if let Some(url) = url {
                            tracing::debug!(%url, error = %e, "artwork download failed");
                            misses.record(&url);
                        }
                    }
                }
            }
        }
    }
    stored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::games;
    use crate::sources::ArtworkCandidate;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0];

    #[test]
    fn detects_image_formats() {
        assert_eq!(image_extension(JPEG), Some("jpg"));
        assert_eq!(image_extension(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(image_extension(b"RIFF\0\0\0\0WEBPVP8"), Some("webp"));
        assert_eq!(image_extension(b"<html>"), None);
        assert_eq!(image_extension(b""), None);
    }

    #[test]
    fn copies_local_files_and_skips_urls_offline() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("library_600x900.jpg");
        std::fs::write(&src, JPEG).unwrap();
        let bad = dir.path().join("broken.jpg");
        std::fs::write(&bad, b"not an image").unwrap();

        let db = Db::open_in_memory().unwrap();
        let game = games::insert(&db.conn(), "A").unwrap();
        let jobs = vec![ArtworkJob {
            game_id: game.id.clone(),
            candidates: vec![
                ArtworkCandidate {
                    kind: ArtworkKind::Cover,
                    sources: vec![
                        ArtworkSource::File(bad),
                        ArtworkSource::File(src),
                        ArtworkSource::Url("https://example.invalid/x.jpg".into()),
                    ],
                },
                ArtworkCandidate {
                    kind: ArtworkKind::Hero,
                    sources: vec![ArtworkSource::Url("https://example.invalid/h.jpg".into())],
                },
            ],
        }];
        let data = dir.path().join("data");
        let misses = MissCache::load(&data);
        assert_eq!(fill(&db, &data, &jobs, false, &misses), 1);
        let stored = artwork::for_game(&db.conn(), &game.id).unwrap();
        let cover = PathBuf::from(&stored["cover"]);
        assert_eq!(cover, cache_root(&data).join(&game.id).join("cover.jpg"));
        assert_eq!(std::fs::read(cover).unwrap(), JPEG);
        assert!(!stored.contains_key("hero"));

        // Already present: nothing more to do.
        assert_eq!(fill(&db, &data, &jobs, false, &misses), 0);
    }

    #[test]
    fn miss_cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let misses = MissCache::load(dir.path());
        misses.record("https://x/a.jpg");
        misses.save();
        let reloaded = MissCache::load(dir.path());
        assert!(reloaded.is_recent_miss("https://x/a.jpg"));
        assert!(!reloaded.is_recent_miss("https://x/b.jpg"));
    }
}
