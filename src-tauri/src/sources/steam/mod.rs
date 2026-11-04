//! Steam source: installed games from local manifests, playtime from the local
//! config, and (with a Web API key) every owned game.
//!
//! Works in degraded modes: no Steam installed but a key → owned games only;
//! Steam installed without a key → installed games only; neither → skipped.

pub mod achievements;
pub mod api;
pub mod local;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::SourceKind;
use crate::sources::{
    unix_to_rfc3339, ArtworkCandidate, ArtworkKind, ArtworkSource, DetectedGame, ScanResult,
    Source, SourceStatus,
};

use local::{AppManifest, LocalPlaytime};

const STORE_ASSETS: &str = "https://shared.steamstatic.com/store_item_assets/steam/apps";

pub fn launch_uri(appid: u64) -> String {
    format!("steam://rungameid/{appid}")
}

/// Everything the Steam source needs, resolved by the caller (settings + vault).
#[derive(Debug, Clone, Default)]
pub struct SteamConfig {
    /// Steam folder, `None` when Steam is not installed.
    pub root: Option<PathBuf>,
    pub steam_id: Option<u64>,
    pub api_key: Option<String>,
}

pub struct SteamSource {
    config: SteamConfig,
}

impl SteamSource {
    pub fn new(config: SteamConfig) -> Self {
        Self { config }
    }

    fn has_api(&self) -> bool {
        self.config.api_key.is_some() && self.config.steam_id.is_some()
    }
}

impl Source for SteamSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Steam
    }

    fn status(&self) -> SourceStatus {
        match (&self.config.root, self.has_api()) {
            (None, false) => SourceStatus::NotInstalled,
            (_, true) => SourceStatus::Connected,
            (Some(_), false) => SourceStatus::NeedsAuth,
        }
    }

    fn scan(&self) -> AppResult<ScanResult> {
        let mut warnings = Vec::new();
        let mut installed = Vec::new();
        let mut playtime = BTreeMap::new();

        if let Some(root) = &self.config.root {
            let (libraries, missing) = local::library_folders(root);
            for lib in missing {
                warnings.push(format!("Steam library not reachable: {}", lib.display()));
            }
            let mut manifests = Vec::new();
            for lib in &libraries {
                manifests.extend(local::installed_apps(lib));
            }
            installed = local::dedupe_manifests(manifests, &local::library_app_owners(root));
            if let Some(account) = self
                .config
                .steam_id
                .and_then(local::steamid64_to_account_id)
            {
                match local::local_playtime(root, account) {
                    Ok(map) => playtime.extend(map),
                    Err(e) => warnings.push(format!("local playtime unavailable: {e}")),
                }
            }
        }

        let mut status = self.status();
        let owned = match (&self.config.api_key, self.config.steam_id) {
            (Some(key), Some(id)) => match api::get_owned_games(key, id) {
                Ok(games) => Some(games),
                Err(e) => {
                    tracing::warn!(error = %e, "Steam Web API failed, using local data only");
                    status = SourceStatus::Error(e.to_string());
                    None
                }
            },
            _ => None,
        };

        let complete_ownership = owned.is_some();
        let games = merge(
            self.config.root.as_deref(),
            &installed,
            &playtime,
            owned.as_deref(),
        );
        Ok(ScanResult {
            kind: SourceKind::Steam,
            status,
            games,
            complete_ownership,
            warnings,
        })
    }
}

fn artwork_for(root: Option<&std::path::Path>, appid: u64) -> Vec<ArtworkCandidate> {
    [
        (ArtworkKind::Cover, local::COVER_FILE),
        (ArtworkKind::Hero, local::HERO_FILE),
        (ArtworkKind::Logo, local::LOGO_FILE),
        (ArtworkKind::Header, local::HEADER_FILE),
    ]
    .into_iter()
    .map(|(kind, file)| {
        let mut sources = Vec::new();
        if let Some(path) = root.and_then(|r| local::find_cached_image(r, appid, file)) {
            sources.push(ArtworkSource::File(path));
        }
        sources.push(ArtworkSource::Url(format!("{STORE_ASSETS}/{appid}/{file}")));
        ArtworkCandidate { kind, sources }
    })
    .collect()
}

/// Combines installed manifests, local playtime and Web API ownership into
/// one entry per appid. The API's counter wins over the local one.
pub fn merge(
    root: Option<&std::path::Path>,
    installed: &[AppManifest],
    local_playtime: &BTreeMap<u64, LocalPlaytime>,
    owned: Option<&[api::OwnedGame]>,
) -> Vec<DetectedGame> {
    let mut games: BTreeMap<u64, DetectedGame> = BTreeMap::new();

    for m in installed {
        let local = local_playtime.get(&m.appid).copied().unwrap_or_default();
        games.insert(
            m.appid,
            DetectedGame {
                external_id: m.appid.to_string(),
                title: m.name.clone(),
                installed: m.fully_installed,
                install_path: Some(m.install_path.display().to_string()),
                size_bytes: m.size_bytes.map(|s| s as i64),
                launch_uri: Some(launch_uri(m.appid)),
                playtime_minutes: local.minutes.map(|m| m as i64),
                last_played: local
                    .last_played
                    .or(m.last_played)
                    .and_then(unix_to_rfc3339),
                artwork: Vec::new(),
            },
        );
    }

    for g in owned.unwrap_or_default() {
        let title = g.name.clone().unwrap_or_default();
        if local::is_steam_tool(g.appid, &title) {
            continue;
        }
        let entry = games.entry(g.appid).or_insert_with(|| DetectedGame {
            external_id: g.appid.to_string(),
            title: title.clone(),
            launch_uri: Some(launch_uri(g.appid)),
            ..Default::default()
        });
        if entry.title.is_empty() {
            entry.title = title;
        }
        entry.playtime_minutes = Some(g.playtime_forever as i64);
        if let Some(date) = unix_to_rfc3339(g.rtime_last_played) {
            entry.last_played = Some(date);
        }
    }

    games
        .into_iter()
        .filter(|(_, g)| !g.title.trim().is_empty())
        .map(|(appid, mut g)| {
            g.artwork = artwork_for(root, appid);
            g
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn manifest(appid: u64, name: &str) -> AppManifest {
        AppManifest {
            appid,
            name: name.into(),
            install_path: PathBuf::from(format!(r"C:\Steam\steamapps\common\{name}")),
            library: PathBuf::from(r"C:\Steam"),
            size_bytes: Some(1000),
            last_played: Some(1_600_000_000),
            last_updated: None,
            fully_installed: true,
        }
    }

    fn owned(appid: u64, name: Option<&str>, minutes: u64, last: u64) -> api::OwnedGame {
        api::OwnedGame {
            appid,
            name: name.map(Into::into),
            playtime_forever: minutes,
            rtime_last_played: last,
        }
    }

    #[test]
    fn installed_only_uses_local_playtime() {
        let mut pt = BTreeMap::new();
        pt.insert(
            10,
            LocalPlaytime {
                minutes: Some(74),
                last_played: Some(1_700_000_000),
            },
        );
        let games = merge(
            None,
            &[manifest(10, "Game A"), manifest(20, "Game B")],
            &pt,
            None,
        );
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].playtime_minutes, Some(74));
        assert_eq!(
            games[0].last_played.as_deref(),
            Some("2023-11-14T22:13:20Z")
        );
        assert_eq!(games[1].playtime_minutes, None);
        // Manifest's LastPlayed is the fallback.
        assert_eq!(
            games[1].last_played.as_deref(),
            Some("2020-09-13T12:26:40Z")
        );
        assert!(games.iter().all(|g| g.installed));
        assert_eq!(games[0].launch_uri.as_deref(), Some("steam://rungameid/10"));
    }

    #[test]
    fn api_adds_owned_games_and_wins_on_playtime() {
        let mut pt = BTreeMap::new();
        pt.insert(
            10,
            LocalPlaytime {
                minutes: Some(5),
                last_played: None,
            },
        );
        let api = [
            owned(10, Some("Game A"), 120, 0),
            owned(30, Some("Not installed"), 60, 1_700_000_000),
            owned(228980, Some("Steamworks Common Redistributables"), 0, 0),
            owned(40, None, 0, 0), // no title anywhere: dropped
        ];
        let games = merge(None, &[manifest(10, "Game A")], &pt, Some(&api));
        let ids: Vec<_> = games.iter().map(|g| g.external_id.as_str()).collect();
        assert_eq!(ids, ["10", "30"]);
        assert_eq!(games[0].playtime_minutes, Some(120));
        assert!(games[0].installed);
        assert!(!games[1].installed);
        assert_eq!(games[1].install_path, None);
    }

    #[test]
    fn artwork_prefers_local_cache_then_cdn() {
        let dir = tempfile::tempdir().unwrap();
        let cover = dir
            .path()
            .join("appcache")
            .join("librarycache")
            .join("10")
            .join(local::COVER_FILE);
        std::fs::create_dir_all(cover.parent().unwrap()).unwrap();
        std::fs::write(&cover, "x").unwrap();
        let games = merge(
            Some(dir.path()),
            &[manifest(10, "A")],
            &BTreeMap::new(),
            None,
        );
        let art = &games[0].artwork;
        let cover_art = art.iter().find(|a| a.kind == ArtworkKind::Cover).unwrap();
        assert_eq!(cover_art.sources[0], ArtworkSource::File(cover));
        assert!(
            matches!(&cover_art.sources[1], ArtworkSource::Url(u) if u.ends_with("/10/library_600x900.jpg"))
        );
        let hero = art.iter().find(|a| a.kind == ArtworkKind::Hero).unwrap();
        assert_eq!(hero.sources.len(), 1); // CDN only
    }

    #[test]
    fn status_reflects_what_is_available() {
        let none = SteamSource::new(SteamConfig::default());
        assert_eq!(none.status(), SourceStatus::NotInstalled);
        let local_only = SteamSource::new(SteamConfig {
            root: Some("C:\\Steam".into()),
            ..Default::default()
        });
        assert_eq!(local_only.status(), SourceStatus::NeedsAuth);
        let api_only = SteamSource::new(SteamConfig {
            steam_id: Some(76561198033149506),
            api_key: Some("k".into()),
            ..Default::default()
        });
        assert_eq!(api_only.status(), SourceStatus::Connected);
    }

    #[test]
    fn scan_without_steam_folder_returns_nothing() {
        let source = SteamSource::new(SteamConfig {
            root: Some(Path::new("Z:\\does-not-exist").into()),
            ..Default::default()
        });
        let result = source.scan().unwrap();
        assert!(result.games.is_empty());
    }
}
