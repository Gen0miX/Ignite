//! Epic Games Launcher source: installed games from the launcher's manifests
//! (`C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item`, JSON).
//! Owned-but-not-installed games need Epic's OAuth API: out of the MVP.
//! Epic exposes no local playtime: time comes from our own tracker.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::SourceKind;
use crate::sources::{DetectedGame, ScanResult, Source, SourceStatus};

pub fn locate_manifests() -> Option<PathBuf> {
    let base = std::env::var_os("PROGRAMDATA").map(PathBuf::from)?;
    let dir = base
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    dir.is_dir().then_some(dir)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: Option<String>,
    app_name: Option<String>,
    install_location: Option<String>,
    launch_executable: Option<String>,
    catalog_namespace: Option<String>,
    catalog_item_id: Option<String>,
    install_size: Option<i64>,
    #[serde(default)]
    app_categories: Vec<String>,
    main_game_app_name: Option<String>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
}

/// `com.epicgames.launcher://apps/<ns>%3A<item>%3A<app>?action=launch&silent=true`,
/// or the short form with the AppName alone when catalog ids are missing.
pub fn launch_uri(app_name: &str, namespace: Option<&str>, item_id: Option<&str>) -> String {
    let id = match (namespace, item_id) {
        (Some(ns), Some(item)) if !ns.is_empty() && !item.is_empty() => {
            format!("{ns}%3A{item}%3A{app_name}")
        }
        _ => app_name.to_string(),
    };
    format!("com.epicgames.launcher://apps/{id}?action=launch&silent=true")
}

/// Parses one `.item` manifest. `Ok(None)` for add-ons, plugins and
/// incomplete installs.
pub fn parse_manifest(text: &str) -> AppResult<Option<DetectedGame>> {
    let m: Manifest = serde_json::from_str(text)?;
    let (Some(app), Some(title)) = (m.app_name.clone(), m.display_name.clone()) else {
        return Ok(None);
    };
    let is_game = m.app_categories.is_empty() || m.app_categories.iter().any(|c| c == "games");
    let is_addon = m
        .main_game_app_name
        .as_deref()
        .is_some_and(|main| main != app);
    if !is_game || is_addon || m.incomplete {
        return Ok(None);
    }
    let install = m.install_location.filter(|p| !p.trim().is_empty());
    let exe = match (&install, m.launch_executable.as_deref()) {
        (Some(dir), Some(exe)) if !exe.trim().is_empty() => {
            Some(Path::new(dir).join(exe).display().to_string())
        }
        _ => None,
    };
    Ok(Some(DetectedGame {
        launch_uri: Some(launch_uri(
            &app,
            m.catalog_namespace.as_deref(),
            m.catalog_item_id.as_deref(),
        )),
        external_id: app,
        title: title.trim().to_string(),
        installed: install.is_some(),
        install_path: install,
        exe_path: exe,
        size_bytes: m.install_size.filter(|s| *s > 0),
        ..Default::default()
    }))
}

pub struct EpicSource {
    manifests: Option<PathBuf>,
}

impl EpicSource {
    pub fn new(manifests: Option<PathBuf>) -> Self {
        Self { manifests }
    }
}

impl Source for EpicSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Epic
    }

    fn status(&self) -> SourceStatus {
        match &self.manifests {
            Some(_) => SourceStatus::Connected,
            None => SourceStatus::NotInstalled,
        }
    }

    fn scan(&self) -> AppResult<ScanResult> {
        let dir = self
            .manifests
            .as_ref()
            .ok_or_else(|| AppError::Invalid("Epic not installed".into()))?;
        let mut games = Vec::new();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("item") {
                continue;
            }
            let parsed = std::fs::read(&path)
                .map_err(AppError::from)
                .and_then(|b| parse_manifest(&String::from_utf8_lossy(&b)));
            match parsed {
                Ok(Some(game)) => games.push(game),
                Ok(None) => {}
                Err(e) => warnings.push(format!("{}: {e}", path.display())),
            }
        }
        games.sort_by(|a, b| a.external_id.cmp(&b.external_id));
        Ok(ScanResult {
            kind: SourceKind::Epic,
            status: SourceStatus::Connected,
            games,
            complete_ownership: false,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM: &str = r#"{
        "FormatVersion": 0,
        "bIsIncompleteInstall": false,
        "LaunchExecutable": "AlanWake2.exe",
        "DisplayName": "Alan Wake 2",
        "InstallLocation": "D:\\Epic Games\\AlanWake2",
        "InstallSize": 90000000000,
        "AppName": "Dill",
        "CatalogNamespace": "dc9d2e595d0e4650b35d659f90d41059",
        "CatalogItemId": "4846b4ca7a8f4e1f8b9f6e2e4dbfc4f8",
        "AppCategories": ["public", "games", "applications"],
        "MainGameAppName": "Dill"
    }"#;

    #[test]
    fn parses_an_installed_game() {
        let g = parse_manifest(ITEM).unwrap().unwrap();
        assert_eq!(g.external_id, "Dill");
        assert_eq!(g.title, "Alan Wake 2");
        assert!(g.installed);
        assert_eq!(
            g.exe_path.as_deref(),
            Some(r"D:\Epic Games\AlanWake2\AlanWake2.exe")
        );
        assert_eq!(
            g.launch_uri.as_deref(),
            Some("com.epicgames.launcher://apps/dc9d2e595d0e4650b35d659f90d41059%3A4846b4ca7a8f4e1f8b9f6e2e4dbfc4f8%3ADill?action=launch&silent=true")
        );
        assert_eq!(g.playtime_minutes, None);
    }

    #[test]
    fn skips_addons_and_incomplete_installs() {
        let addon = ITEM.replace(
            r#""MainGameAppName": "Dill""#,
            r#""MainGameAppName": "Other""#,
        );
        assert_eq!(parse_manifest(&addon).unwrap(), None);
        let plugin = ITEM.replace(
            r#"["public", "games", "applications"]"#,
            r#"["plugins", "engine"]"#,
        );
        assert_eq!(parse_manifest(&plugin).unwrap(), None);
        let partial = ITEM.replace(
            r#""bIsIncompleteInstall": false"#,
            r#""bIsIncompleteInstall": true"#,
        );
        assert_eq!(parse_manifest(&partial).unwrap(), None);
        assert!(parse_manifest("not json").is_err());
    }

    #[test]
    fn short_launch_uri_without_catalog_ids() {
        assert_eq!(
            launch_uri("Fortnite", None, None),
            "com.epicgames.launcher://apps/Fortnite?action=launch&silent=true"
        );
    }

    #[test]
    fn scans_a_folder_and_reports_bad_files() {
        let dir = std::env::temp_dir().join(format!("ludotheque-epic-{}", crate::db::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("A.item"), ITEM).unwrap();
        std::fs::write(dir.join("B.item"), "{ broken").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
        let result = EpicSource::new(Some(dir.clone())).scan().unwrap();
        assert_eq!(result.games.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(EpicSource::new(None).status(), SourceStatus::NotInstalled);
    }
}
