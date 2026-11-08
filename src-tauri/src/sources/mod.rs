//! Game sources (Steam, Epic, GOG, ...).
//!
//! Every launcher is optional: a source whose launcher is missing reports
//! `SourceStatus::NotInstalled` and returns no games. A failing source never
//! prevents the others from being scanned (see `scan_all`).

pub mod captures;
pub mod emulator;
pub mod epic;
pub mod folder_scan;
pub mod gog;
pub mod local;
pub mod others;
pub mod steam;
pub mod uninstall;
pub mod vdf;

use serde::Serialize;

use crate::error::AppResult;
use crate::models::SourceKind;

/// Connection state shown in the sidebar and in Settings › Sources.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum SourceStatus {
    /// Launcher found and readable.
    Connected,
    /// Launcher not installed on this PC: not an error, the source is skipped.
    NotInstalled,
    /// Readable, but an account or key is needed for part of the data.
    NeedsAuth,
    /// Something went wrong (message for the logs / settings card).
    Error(String),
}

/// Artwork kinds stored in the cache (ARCHITECTURE.md § 3, table `artwork`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ArtworkKind {
    Cover,
    Hero,
    Logo,
    Header,
}

impl ArtworkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtworkKind::Cover => "cover",
            ArtworkKind::Hero => "hero",
            ArtworkKind::Logo => "logo",
            ArtworkKind::Header => "header",
        }
    }
}

/// Where an image for a detected game can be obtained.
#[derive(Debug, Clone, PartialEq)]
pub enum ArtworkSource {
    /// A file on disk (e.g. Steam's own library cache), copied into our cache.
    File(std::path::PathBuf),
    /// A URL downloaded into our cache (never hotlinked).
    Url(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtworkCandidate {
    pub kind: ArtworkKind,
    /// Tried in order; the first that works wins.
    pub sources: Vec<ArtworkSource>,
}

/// A game found by a source scan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetectedGame {
    pub external_id: String,
    pub title: String,
    pub installed: bool,
    pub install_path: Option<String>,
    /// Executable to start (direct launch / GOG play task / launcher client).
    pub exe_path: Option<String>,
    /// Arguments for `exe_path` (e.g. Riot's `--launch-product=...`).
    pub launch_args: Option<String>,
    /// Process matchers JSON suggested by the source (user choices win).
    pub process_matchers: Option<String>,
    pub size_bytes: Option<i64>,
    pub launch_uri: Option<String>,
    /// Playtime counted by the platform itself, in minutes.
    pub playtime_minutes: Option<i64>,
    /// RFC 3339 date of the last session according to the platform.
    pub last_played: Option<String>,
    pub artwork: Vec<ArtworkCandidate>,
    /// Genres provided by the source (normalized French names).
    pub genres: Vec<String>,
    /// Import hidden (DLC variant, tool...). The user can show it again.
    pub hide_on_import: bool,
    /// Achievements provided by the source itself (GOG database);
    /// `None` = not provided by the scan.
    pub achievements: Option<Vec<crate::db::repo::achievements::AchievementRow>>,
}

/// Result of scanning one source.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub kind: SourceKind,
    pub status: SourceStatus,
    pub games: Vec<DetectedGame>,
    /// True when `games` lists everything owned (not only what is installed);
    /// lets the sync know that a missing game is really gone.
    pub complete_ownership: bool,
    /// Non-fatal problems (e.g. a library on an unplugged drive).
    pub warnings: Vec<String>,
}

impl ScanResult {
    pub fn empty(kind: SourceKind, status: SourceStatus) -> Self {
        Self {
            kind,
            status,
            games: Vec::new(),
            complete_ownership: false,
            warnings: Vec::new(),
        }
    }
}

pub trait Source: Send + Sync {
    fn kind(&self) -> SourceKind;
    /// Cheap check, no network.
    fn status(&self) -> SourceStatus;
    /// Lists games. May use the network; called from a blocking thread.
    fn scan(&self) -> AppResult<ScanResult>;
}

/// Scans every source independently: an error in one becomes its
/// `SourceStatus::Error` and the others still run.
pub fn scan_all(sources: &[Box<dyn Source>]) -> Vec<ScanResult> {
    sources
        .iter()
        .map(|source| {
            let kind = source.kind();
            match source.status() {
                SourceStatus::NotInstalled => {
                    tracing::info!(?kind, "source not installed, skipped");
                    ScanResult::empty(kind, SourceStatus::NotInstalled)
                }
                _ => match source.scan() {
                    Ok(result) => result,
                    Err(e) => {
                        tracing::warn!(?kind, error = %e, "source scan failed");
                        ScanResult::empty(kind, SourceStatus::Error(e.to_string()))
                    }
                },
            }
        })
        .collect()
}

/// Unix timestamp (seconds) → RFC 3339, ignoring Steam's "0 = never".
pub fn unix_to_rfc3339(ts: u64) -> Option<String> {
    if ts == 0 {
        return None;
    }
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    struct Fake(SourceKind, SourceStatus, Option<&'static str>);

    impl Source for Fake {
        fn kind(&self) -> SourceKind {
            self.0
        }
        fn status(&self) -> SourceStatus {
            self.1.clone()
        }
        fn scan(&self) -> AppResult<ScanResult> {
            match self.2 {
                Some(msg) => Err(AppError::Invalid(msg.into())),
                None => Ok(ScanResult {
                    games: vec![DetectedGame {
                        external_id: "1".into(),
                        title: "Game".into(),
                        ..Default::default()
                    }],
                    ..ScanResult::empty(self.0, SourceStatus::Connected)
                }),
            }
        }
    }

    #[test]
    fn missing_or_failing_sources_do_not_block_others() {
        let sources: Vec<Box<dyn Source>> = vec![
            Box::new(Fake(
                SourceKind::Epic,
                SourceStatus::NotInstalled,
                Some("must not be called"),
            )),
            Box::new(Fake(
                SourceKind::Gog,
                SourceStatus::Connected,
                Some("db locked"),
            )),
            Box::new(Fake(SourceKind::Steam, SourceStatus::Connected, None)),
        ];
        let results = scan_all(&sources);
        assert_eq!(results[0].status, SourceStatus::NotInstalled);
        assert!(results[0].games.is_empty());
        assert!(matches!(&results[1].status, SourceStatus::Error(m) if m.contains("db locked")));
        assert_eq!(results[2].status, SourceStatus::Connected);
        assert_eq!(results[2].games.len(), 1);
    }

    #[test]
    fn no_sources_at_all_is_fine() {
        assert!(scan_all(&[]).is_empty());
    }

    #[test]
    fn unix_timestamps_convert() {
        assert_eq!(unix_to_rfc3339(0), None);
        assert_eq!(
            unix_to_rfc3339(1_700_000_000).as_deref(),
            Some("2023-11-14T22:13:20Z")
        );
    }
}
