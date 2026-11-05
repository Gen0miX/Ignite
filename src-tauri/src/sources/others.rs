//! Secondary launchers (INTEGRATIONS.md): Riot, Battle.net, EA app and
//! Ubisoft Connect. Installed games only, detected from the Windows
//! "installed programs" list; playtime comes from Ludothèque's tracker.

use std::path::Path;

use crate::error::AppResult;
use crate::models::SourceKind;
use crate::sources::uninstall::{clean_path, icon_path, UninstallEntry};
use crate::sources::{DetectedGame, ScanResult, Source, SourceStatus};
use crate::tracker::matcher::ProcessMatcher;

fn matchers_json(list: &[ProcessMatcher]) -> Option<String> {
    (!list.is_empty())
        .then(|| serde_json::to_string(list).ok())
        .flatten()
}

fn existing_dir(p: Option<&str>) -> Option<String> {
    p.map(clean_path).filter(|p| !p.is_empty())
}

// ---------- Riot ----------

/// `Riot Game valorant.live` → (`valorant`, `live`).
fn riot_product(key: &str) -> Option<(String, String)> {
    let id = key.strip_prefix("Riot Game ")?;
    let (product, patchline) = id.split_once('.')?;
    (!product.is_empty() && !patchline.is_empty() && product != "Riot_Client")
        .then(|| (product.to_string(), patchline.to_string()))
}

/// Game processes (not the Riot client or the lobby), so lobby time is not
/// counted as play time.
fn riot_matchers(product: &str) -> Vec<ProcessMatcher> {
    let exe = |n: &str| vec![ProcessMatcher::Exe(n.into())];
    match product {
        "league_of_legends" => exe("League of Legends.exe"),
        "valorant" => exe("VALORANT-Win64-Shipping.exe"),
        "bacon" => exe("LoR.exe"),
        // Teamfight Tactics runs inside League of Legends' game process.
        "teamfighttactics" => Vec::new(),
        _ => Vec::new(),
    }
}

pub fn riot_games(entries: &[UninstallEntry], client: Option<&str>) -> Vec<DetectedGame> {
    let mut games: Vec<DetectedGame> = entries
        .iter()
        .filter_map(|e| {
            let (product, patchline) = riot_product(&e.key)?;
            if patchline != "live" {
                return None; // PBE and test patchlines
            }
            let install = existing_dir(e.install_location.as_deref());
            let mut matchers = riot_matchers(&product);
            if matchers.is_empty() && product != "teamfighttactics" {
                if let Some(dir) = &install {
                    matchers.push(ProcessMatcher::PathPrefix(dir.clone()));
                }
            }
            Some(DetectedGame {
                external_id: format!("{product}.{patchline}"),
                title: e.display_name.trim().to_string(),
                installed: true,
                install_path: install,
                exe_path: client.map(str::to_string),
                launch_args: Some(format!(
                    "--launch-product={product} --launch-patchline={patchline}"
                )),
                process_matchers: matchers_json(&matchers),
                ..Default::default()
            })
        })
        .collect();
    games.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    games
}

/// `RiotClientServices.exe` from `C:\ProgramData\Riot Games\RiotClientInstalls.json`.
pub fn riot_client() -> Option<String> {
    let base = std::env::var_os("PROGRAMDATA")?;
    let file = Path::new(&base)
        .join("Riot Games")
        .join("RiotClientInstalls.json");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    let exe = json.get("rc_live").or(json.get("rc_default"))?.as_str()?;
    let path = clean_path(exe);
    Path::new(&path).is_file().then_some(path)
}

// ---------- Battle.net ----------

/// Battle.net product codes for `battlenet://<code>`, from the `--uid=`
/// of the uninstall string (prefix before `_` locale suffix).
fn battlenet_code(uid: &str) -> Option<&'static str> {
    let base = uid.split('_').next().unwrap_or(uid).to_lowercase();
    Some(match base.as_str() {
        "wow" => "WoW",
        "wow_classic" | "wowclassic" => "WoWC",
        "diablo3" | "d3" => "D3",
        "diablo4" | "fenris" => "Fen",
        "prometheus" | "overwatch" => "Pro",
        "hs_beta" | "hearthstone" => "WTCG",
        "heroes" | "hero" => "Hero",
        "s2" | "starcraft2" => "S2",
        "s1" | "starcraft" => "S1",
        "w3" | "warcraft3" => "W3",
        "odin" | "warzone" => "ODIN",
        "viper" => "VIPR",
        "lazarus" => "LAZR",
        "zeus" => "ZEUS",
        "fore" => "FORE",
        "auks" => "AUKS",
        "spot" => "SPOT",
        _ => return None,
    })
}

fn uid_of(uninstall: &str) -> Option<String> {
    let i = uninstall.find("--uid=")?;
    let rest = &uninstall[i + 6..];
    Some(
        rest.split(|c: char| c.is_whitespace() || c == '"')
            .next()?
            .to_string(),
    )
}

pub fn battlenet_games(entries: &[UninstallEntry]) -> Vec<DetectedGame> {
    let mut games: Vec<DetectedGame> = entries
        .iter()
        .filter(|e| e.publisher.to_lowercase().contains("blizzard"))
        // Blizzard games bought on Steam are Steam games; the app itself is not a game.
        .filter(|e| {
            !e.key.starts_with("Steam App ")
                && !e.display_name.to_lowercase().starts_with("battle.net")
        })
        .filter_map(|e| {
            let uid = e.uninstall_string.as_deref().and_then(uid_of)?;
            let code = battlenet_code(&uid);
            let install = existing_dir(e.install_location.as_deref());
            Some(DetectedGame {
                external_id: uid.clone(),
                title: e
                    .display_name
                    .trim()
                    .trim_end_matches('®')
                    .trim()
                    .to_string(),
                installed: true,
                launch_uri: code.map(|c| format!("battlenet://{c}")),
                exe_path: e
                    .display_icon
                    .as_deref()
                    .map(icon_path)
                    .filter(|p| p.to_lowercase().ends_with(".exe")),
                process_matchers: install
                    .as_ref()
                    .and_then(|d| matchers_json(&[ProcessMatcher::PathPrefix(d.clone())])),
                install_path: install,
                ..Default::default()
            })
        })
        .collect();
    games.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    games
}

// ---------- EA app ----------

pub fn ea_games(entries: &[UninstallEntry]) -> Vec<DetectedGame> {
    let mut games: Vec<DetectedGame> = entries
        .iter()
        .filter(|e| {
            let p = e.publisher.to_lowercase();
            p.contains("electronic arts") || p == "ea" || p.starts_with("ea ")
        })
        .filter(|e| !e.key.starts_with("Steam App "))
        .filter(|e| {
            let n = e.display_name.to_lowercase();
            !(n == "ea app"
                || n.starts_with("ea app ")
                || n.contains("origin")
                || n.contains("anti-cheat"))
        })
        .filter_map(|e| {
            let install = existing_dir(e.install_location.as_deref())?;
            let exe = e
                .display_icon
                .as_deref()
                .map(icon_path)
                .filter(|p| p.to_lowercase().ends_with(".exe"));
            Some(DetectedGame {
                external_id: e.key.clone(),
                title: e.display_name.trim().to_string(),
                installed: true,
                exe_path: exe,
                process_matchers: matchers_json(&[ProcessMatcher::PathPrefix(install.clone())]),
                install_path: Some(install),
                ..Default::default()
            })
        })
        .collect();
    games.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    games
}

// ---------- Ubisoft Connect ----------

/// `(id, install dir)` from `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs`.
pub fn ubisoft_installs() -> Vec<(String, String)> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs")
        else {
            return Vec::new();
        };
        root.enum_keys()
            .flatten()
            .filter_map(|id| {
                let dir: String = root.open_subkey(&id).ok()?.get_value("InstallDir").ok()?;
                Some((id, dir))
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn ubisoft_games(
    installs: &[(String, String)],
    entries: &[UninstallEntry],
) -> Vec<DetectedGame> {
    let mut games: Vec<DetectedGame> = installs
        .iter()
        .filter_map(|(id, dir)| {
            let dir = clean_path(dir);
            if dir.is_empty() {
                return None;
            }
            // "Uplay Install <id>" carries the display name; else the folder name.
            let title = entries
                .iter()
                .find(|e| e.key == format!("Uplay Install {id}"))
                .map(|e| e.display_name.clone())
                .or_else(|| {
                    Path::new(&dir)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                })?;
            Some(DetectedGame {
                external_id: id.clone(),
                title,
                installed: true,
                launch_uri: Some(format!("uplay://launch/{id}/0")),
                process_matchers: matchers_json(&[ProcessMatcher::PathPrefix(dir.clone())]),
                install_path: Some(dir),
                ..Default::default()
            })
        })
        .collect();
    games.sort_by(|a, b| a.external_id.cmp(&b.external_id));
    games
}

// ---------- Source implementations ----------

/// A launcher read from the uninstall list (and its own files).
pub struct OtherSource {
    kind: SourceKind,
}

impl OtherSource {
    pub fn new(kind: SourceKind) -> Self {
        Self { kind }
    }

    fn detect(&self) -> Vec<DetectedGame> {
        let entries = crate::sources::uninstall::read_all();
        match self.kind {
            SourceKind::Riot => riot_games(&entries, riot_client().as_deref()),
            SourceKind::Battlenet => battlenet_games(&entries),
            SourceKind::Ea => ea_games(&entries),
            SourceKind::Ubisoft => ubisoft_games(&ubisoft_installs(), &entries),
            _ => Vec::new(),
        }
    }

    /// Launcher present on this PC (its client, not necessarily games).
    fn launcher_present(&self) -> bool {
        let pd = std::env::var_os("PROGRAMDATA").map(std::path::PathBuf::from);
        match self.kind {
            SourceKind::Riot => riot_client().is_some(),
            SourceKind::Battlenet => pd.is_some_and(|p| p.join("Battle.net").is_dir()),
            SourceKind::Ea => pd.is_some_and(|p| p.join("EA Desktop").is_dir()),
            SourceKind::Ubisoft => {
                !ubisoft_installs().is_empty()
                    || crate::sources::uninstall::read_all()
                        .iter()
                        .any(|e| e.display_name.contains("Ubisoft Connect"))
            }
            _ => false,
        }
    }
}

impl Source for OtherSource {
    fn kind(&self) -> SourceKind {
        self.kind
    }

    fn status(&self) -> SourceStatus {
        if self.launcher_present() {
            SourceStatus::Connected
        } else {
            SourceStatus::NotInstalled
        }
    }

    fn scan(&self) -> AppResult<ScanResult> {
        Ok(ScanResult {
            kind: self.kind,
            status: SourceStatus::Connected,
            games: self.detect(),
            complete_ownership: false,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, name: &str, publisher: &str) -> UninstallEntry {
        UninstallEntry {
            key: key.into(),
            display_name: name.into(),
            publisher: publisher.into(),
            ..Default::default()
        }
    }

    #[test]
    fn riot_games_from_the_users_registry_layout() {
        let mut lol = entry(
            "Riot Game league_of_legends.live",
            "League of Legends",
            "Riot Games, Inc",
        );
        lol.install_location = Some("C:/Riot Games/League of Legends".into());
        let mut val = entry("Riot Game valorant.live", "VALORANT", "Riot Games, Inc");
        val.install_location = Some("C:/Riot Games/VALORANT/live".into());
        let mut xko = entry("Riot Game lion.live", "2XKO", "Riot Games, Inc");
        xko.install_location = Some("C:/Riot Games/2XKO/Live".into());
        let entries = vec![
            lol,
            val,
            xko,
            entry(
                "Riot Game teamfighttactics.live",
                "Teamfight Tactics",
                "Riot Games, Inc",
            ),
            entry(
                "Riot Game league_of_legends.pbe",
                "League PBE",
                "Riot Games, Inc",
            ),
            entry("Riot Game Riot_Client.", "Riot Client", "Riot Games, Inc"),
            entry("Riot Vanguard", "Riot Vanguard", "Riot Games, Inc."),
        ];
        let client = r"C:\Riot Games\Riot Client\RiotClientServices.exe";
        let games = riot_games(&entries, Some(client));
        let ids: Vec<_> = games.iter().map(|g| g.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "league_of_legends.live",
                "lion.live",
                "teamfighttactics.live",
                "valorant.live"
            ]
        );
        let lol = &games[0];
        assert_eq!(
            lol.install_path.as_deref(),
            Some(r"C:\Riot Games\League of Legends")
        );
        assert_eq!(lol.exe_path.as_deref(), Some(client));
        assert_eq!(
            lol.launch_args.as_deref(),
            Some("--launch-product=league_of_legends --launch-patchline=live")
        );
        // Only the game process, not the client/lobby.
        assert_eq!(
            lol.process_matchers.as_deref(),
            Some(r#"[{"exe":"League of Legends.exe"}]"#)
        );
        // Unknown product: its folder.
        assert!(games[1]
            .process_matchers
            .as_deref()
            .unwrap()
            .contains("path_prefix"));
        assert_eq!(games[2].process_matchers, None);
    }

    #[test]
    fn battlenet_games_skip_steam_copies_and_the_app() {
        let mut d4 = entry("Diablo IV", "Diablo IV®", "Blizzard Entertainment");
        d4.install_location = Some(r"C:\Program Files (x86)\Diablo IV".into());
        d4.uninstall_string = Some(r#""C:\ProgramData\Battle.net\Agent\Blizzard Uninstaller.exe" --lang=frFR --uid=fenris --displayname="Diablo IV""#.into());
        d4.display_icon = Some(r#"C:\Program Files (x86)\Diablo IV\Diablo IV Launcher.exe"#.into());
        let mut ow_steam = entry(
            "Steam App 2357570",
            "Overwatch®",
            "Blizzard Entertainment, Inc.",
        );
        ow_steam.uninstall_string = Some("steam://uninstall/2357570".into());
        let app = entry("Battle.net", "Battle.net", "Blizzard Entertainment");
        let games = battlenet_games(&[d4, ow_steam, app]);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Diablo IV");
        assert_eq!(games[0].launch_uri.as_deref(), Some("battlenet://Fen"));
        assert_eq!(games[0].external_id, "fenris");
        assert!(games[0]
            .exe_path
            .as_deref()
            .unwrap()
            .ends_with("Diablo IV Launcher.exe"));
    }

    #[test]
    fn ea_games_need_an_install_folder() {
        let mut fc = entry("{1234}", "EA SPORTS FC 25", "Electronic Arts");
        fc.install_location = Some(r"C:\Program Files\EA Games\EA SPORTS FC 25".into());
        fc.display_icon = Some(r"C:\Program Files\EA Games\EA SPORTS FC 25\FC25.exe,0".into());
        let app = entry("{EA app}", "EA app", "Electronic Arts");
        let no_dir = entry("{x}", "Some EA Tool", "Electronic Arts");
        let games = ea_games(&[fc, app, no_dir]);
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].exe_path.as_deref(),
            Some(r"C:\Program Files\EA Games\EA SPORTS FC 25\FC25.exe")
        );
    }

    #[test]
    fn ubisoft_games_from_installs() {
        let names = vec![entry(
            "Uplay Install 635",
            "Assassin's Creed Origins",
            "Ubisoft",
        )];
        let games = ubisoft_games(
            &[
                (
                    "635".into(),
                    "C:/Program Files (x86)/Ubisoft/Games/ACO/".into(),
                ),
                ("9".into(), "D:/Ubi/Anno 1800".into()),
            ],
            &names,
        );
        assert_eq!(games[0].title, "Assassin's Creed Origins");
        assert_eq!(games[0].launch_uri.as_deref(), Some("uplay://launch/635/0"));
        assert_eq!(
            games[0].install_path.as_deref(),
            Some(r"C:\Program Files (x86)\Ubisoft\Games\ACO")
        );
        assert_eq!(games[1].title, "Anno 1800");
    }

    /// Detects the launchers of this PC (read-only).
    /// Run with `cargo test others -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the local registry"]
    fn detects_this_pcs_launchers() {
        for kind in [
            SourceKind::Riot,
            SourceKind::Battlenet,
            SourceKind::Ea,
            SourceKind::Ubisoft,
        ] {
            let s = OtherSource::new(kind);
            let games = if s.status() == SourceStatus::Connected {
                s.scan().unwrap().games
            } else {
                vec![]
            };
            println!(
                "{kind:?}: {:?} {:?}",
                s.status(),
                games
                    .iter()
                    .map(|g| (&g.title, &g.process_matchers))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn battlenet_codes() {
        assert_eq!(battlenet_code("fenris"), Some("Fen"));
        assert_eq!(battlenet_code("wow_frfr"), Some("WoW"));
        assert_eq!(battlenet_code("unknown"), None);
        assert_eq!(
            uid_of(r#"x.exe --lang=frFR --uid=prometheus --displayname="Overwatch""#).as_deref(),
            Some("prometheus")
        );
    }
}
