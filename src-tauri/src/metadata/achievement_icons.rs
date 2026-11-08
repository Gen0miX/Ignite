//! Achievement icons, copied into the image cache (`artwork/achievements/`)
//! and served through `asset:` like the other images: no hotlinking. A file is
//! named after a hash of its URL, so a resync of the list keeps the cache.

use std::path::{Path, PathBuf};

use super::artwork::{cache_root, download_named};

/// Hosts the icons come from (Steam, GOG, RetroAchievements).
fn allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host == "steamcdn-a.akamaihd.net"
        || host.ends_with(".steamstatic.com")
        || host == "images.gog.com"
        || host.ends_with(".gog-statics.com")
        || host == "media.retroachievements.org"
}

/// FNV-1a 64: stable across runs and Rust versions (unlike `DefaultHasher`).
pub(crate) fn file_stem(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn dir(data_dir: &Path) -> PathBuf {
    cache_root(data_dir).join("achievements")
}

/// Local copy of an icon, if already downloaded.
pub fn cached(dir: &Path, url: &str) -> Option<PathBuf> {
    let stem = file_stem(url);
    ["jpg", "png", "webp"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

/// Downloads the icons not cached yet; returns how many were added. A failed
/// icon is skipped (the UI shows a trophy instead).
pub fn fetch_missing(dir: &Path, urls: &[String]) -> usize {
    let mut added = 0;
    for url in urls {
        if !allowed(url) || cached(dir, url).is_some() {
            continue;
        }
        match download_named(dir, &file_stem(url), url) {
            Ok(_) => added += 1,
            Err(e) => tracing::debug!(%url, error = %e, "achievement icon not downloaded"),
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_cdns_and_stable_names() {
        assert!(allowed(
            "https://steamcdn-a.akamaihd.net/steamcommunity/public/images/apps/1/a.jpg"
        ));
        assert!(allowed(
            "https://cdn.akamai.steamstatic.com/steamcommunity/a.jpg"
        ));
        assert!(allowed("https://images.gog.com/x_gac_60.jpg"));
        assert!(allowed("https://media.retroachievements.org/Badge/001.png"));
        assert!(!allowed("http://images.gog.com/x.jpg"));
        assert!(!allowed("https://evil.example/steamstatic.com/a.jpg"));
        assert_eq!(file_stem("https://a/b.jpg"), file_stem("https://a/b.jpg"));
        assert_ne!(file_stem("https://a/b.jpg"), file_stem("https://a/c.jpg"));
        assert_eq!(file_stem("").len(), 16);
    }

    #[test]
    fn finds_a_cached_file_whatever_its_format() {
        let dir = std::env::temp_dir().join(format!("ignite-ach-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = "https://images.gog.com/x.jpg";
        assert!(cached(&dir, url).is_none());
        std::fs::write(dir.join(format!("{}.png", file_stem(url))), b"x").unwrap();
        assert!(cached(&dir, url).is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
