//! Read-only access to the local Steam installation: libraries, installed
//! games, per-user playtime and accounts. Never writes to Steam's files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::sources::vdf::{self, Vdf};

/// Offset between a SteamID64 and the account id used in `userdata\<id>`.
pub const STEAMID64_BASE: u64 = 76_561_197_960_265_728;

pub fn steamid64_to_account_id(id64: u64) -> Option<u64> {
    id64.checked_sub(STEAMID64_BASE)
        .filter(|id| *id > 0 && *id < (1 << 32))
}

/// Parses a SteamID64 typed by the user (17 digits, individual account range).
pub fn parse_steamid64(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.len() != 17 || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id: u64 = t.parse().ok()?;
    steamid64_to_account_id(id).map(|_| id)
}

/// A Steam folder is usable when it contains `steamapps`.
pub fn is_steam_root(path: &Path) -> bool {
    path.join("steamapps").is_dir()
}

/// Registry values use forward slashes and lowercase (`c:/program files (x86)/steam`).
pub fn normalize_registry_path(raw: &str) -> PathBuf {
    PathBuf::from(raw.trim().replace('/', "\\"))
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('\\').to_lowercase()
        == b.to_string_lossy().trim_end_matches('\\').to_lowercase()
}

/// Picks the Steam folder from candidates in priority order. A lowercase path
/// from HKCU is replaced by an equivalent, properly cased candidate.
pub fn pick_steam_root(user_path: Option<PathBuf>, others: Vec<PathBuf>) -> Option<PathBuf> {
    if let Some(user) = user_path.filter(|p| is_steam_root(p)) {
        let nicer = others.iter().find(|o| same_path(o, &user)).cloned();
        return Some(nicer.unwrap_or(user));
    }
    others.into_iter().find(|p| is_steam_root(p))
}

/// Finds the Steam installation, or `None` when Steam is not installed.
pub fn locate_steam() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;
        let read = |hive, key: &str, value: &str| -> Option<String> {
            RegKey::predef(hive)
                .open_subkey(key)
                .ok()?
                .get_value::<String, _>(value)
                .ok()
        };
        let user = read(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath")
            .map(|s| normalize_registry_path(&s));
        let mut others = Vec::new();
        for key in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
            if let Some(p) = read(HKEY_LOCAL_MACHINE, key, "InstallPath") {
                others.push(normalize_registry_path(&p));
            }
        }
        others.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        pick_steam_root(user, others)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn read_vdf(path: &Path) -> AppResult<Vdf> {
    let bytes = std::fs::read(path)?;
    vdf::parse(&String::from_utf8_lossy(&bytes))
}

/// Library folders declared in `steamapps\libraryfolders.vdf` (Steam root
/// always included). Returns `(existing, missing)`: a library on an unplugged
/// drive is reported, not treated as an error.
pub fn library_folders(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut all = vec![root.to_path_buf()];
    match read_vdf(&root.join("steamapps").join("libraryfolders.vdf")) {
        Ok(doc) => {
            let folders = doc.get("libraryfolders").unwrap_or(&doc);
            for (_, entry) in folders.entries() {
                if let Some(path) = entry.get_str("path") {
                    let p = PathBuf::from(path);
                    if !all.iter().any(|a| same_path(a, &p)) {
                        all.push(p);
                    }
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "cannot read libraryfolders.vdf"),
    }
    all.into_iter().partition(|p| p.join("steamapps").is_dir())
}

/// `appid → library` according to the `"apps"` block of each library in
/// `libraryfolders.vdf`. This is Steam's own view of where a game lives and
/// settles stale manifests left in another library.
pub fn library_app_owners(root: &Path) -> HashMap<u64, PathBuf> {
    match read_vdf(&root.join("steamapps").join("libraryfolders.vdf")) {
        Ok(doc) => parse_library_app_owners(&doc),
        Err(_) => HashMap::new(),
    }
}

pub fn parse_library_app_owners(doc: &Vdf) -> HashMap<u64, PathBuf> {
    let folders = doc.get("libraryfolders").unwrap_or(doc);
    let mut owners = HashMap::new();
    for (_, entry) in folders.entries() {
        let Some(path) = entry.get_str("path") else {
            continue;
        };
        for (appid, _) in entry.get("apps").map(Vdf::entries).unwrap_or_default() {
            if let Ok(id) = appid.parse() {
                owners.insert(id, PathBuf::from(path));
            }
        }
    }
    owners
}

/// One manifest per appid when a game has manifests in several libraries
/// (an old copy left behind after moving the game). Preference: the library
/// Steam lists the game in, then fully installed, then most recently updated.
pub fn dedupe_manifests(
    manifests: Vec<AppManifest>,
    owners: &HashMap<u64, PathBuf>,
) -> Vec<AppManifest> {
    let mut best: HashMap<u64, AppManifest> = HashMap::new();
    let rank = |m: &AppManifest| {
        let owned = owners
            .get(&m.appid)
            .is_some_and(|lib| same_path(lib, &m.library));
        (owned, m.fully_installed, m.last_updated.unwrap_or(0))
    };
    for m in manifests {
        match best.get(&m.appid) {
            Some(current) if rank(current) >= rank(&m) => {
                tracing::info!(appid = m.appid, ignored = %m.library.display(), "duplicate Steam manifest ignored");
            }
            _ => {
                best.insert(m.appid, m);
            }
        }
    }
    let mut list: Vec<_> = best.into_values().collect();
    list.sort_by_key(|m| m.appid);
    list
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppManifest {
    pub appid: u64,
    pub name: String,
    pub install_path: PathBuf,
    /// Library folder the manifest was read from.
    pub library: PathBuf,
    pub size_bytes: Option<u64>,
    pub last_played: Option<u64>,
    pub last_updated: Option<u64>,
    pub fully_installed: bool,
}

/// StateFlags bit meaning "fully installed" (an update may be pending).
const STATE_FULLY_INSTALLED: u64 = 4;

/// Tools Steam installs alongside games; never shown as games.
pub fn is_steam_tool(appid: u64, name: &str) -> bool {
    const TOOL_PREFIXES: &[&str] = &[
        "Proton ",
        "Proton-",
        "Steam Linux Runtime",
        "Steamworks Common Redistributables",
    ];
    appid == 228_980 || name == "Proton" || TOOL_PREFIXES.iter().any(|p| name.starts_with(p))
}

pub fn parse_manifest(text: &str, library: &Path) -> AppResult<Option<AppManifest>> {
    let doc = vdf::parse(text)?;
    let Some(state) = doc.get("AppState") else {
        return Ok(None);
    };
    let (Some(appid), Some(name)) = (state.get_u64("appid"), state.get_str("name")) else {
        return Ok(None);
    };
    let installdir = state.get_str("installdir").unwrap_or_default();
    Ok(Some(AppManifest {
        appid,
        name: name.trim().to_string(),
        install_path: library.join("steamapps").join("common").join(installdir),
        library: library.to_path_buf(),
        last_updated: state.get_u64("LastUpdated").filter(|t| *t > 0),
        size_bytes: state.get_u64("SizeOnDisk").filter(|s| *s > 0),
        last_played: state.get_u64("LastPlayed").filter(|t| *t > 0),
        fully_installed: state.get_u64("StateFlags").unwrap_or(0) & STATE_FULLY_INSTALLED != 0,
    }))
}

/// Installed games of one library (`appmanifest_*.acf`), tools excluded.
/// Unreadable manifests are skipped with a warning.
pub fn installed_apps(library: &Path) -> Vec<AppManifest> {
    let dir = library.join("steamapps");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut apps = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
            continue;
        }
        let parsed = std::fs::read(entry.path())
            .map_err(Into::into)
            .and_then(|b| parse_manifest(&String::from_utf8_lossy(&b), library));
        match parsed {
            Ok(Some(m)) if !is_steam_tool(m.appid, &m.name) => apps.push(m),
            Ok(Some(_)) => {}
            Ok(None) => tracing::info!(file = %name, "empty or corrupted manifest skipped"),
            Err(e) => tracing::warn!(file = %name, error = %e, "skipping unreadable manifest"),
        }
    }
    apps.sort_by_key(|m| m.appid);
    apps
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalPlaytime {
    pub minutes: Option<u64>,
    pub last_played: Option<u64>,
}

/// Per-game playtime Steam keeps locally for one account
/// (`userdata\<account>\config\localconfig.vdf`, minutes). Fallback when no
/// Web API key is configured.
pub fn local_playtime(root: &Path, account_id: u64) -> AppResult<HashMap<u64, LocalPlaytime>> {
    let path = root
        .join("userdata")
        .join(account_id.to_string())
        .join("config")
        .join("localconfig.vdf");
    let doc = read_vdf(&path)?;
    Ok(parse_local_playtime(&doc))
}

pub fn parse_local_playtime(doc: &Vdf) -> HashMap<u64, LocalPlaytime> {
    let Some(apps) = doc.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"])
    else {
        return HashMap::new();
    };
    apps.entries()
        .iter()
        .filter_map(|(id, node)| {
            let appid: u64 = id.parse().ok()?;
            let entry = LocalPlaytime {
                minutes: node.get_u64("Playtime"),
                last_played: node.get_u64("LastPlayed").filter(|t| *t > 0),
            };
            (entry.minutes.is_some() || entry.last_played.is_some()).then_some((appid, entry))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SteamAccount {
    pub steam_id: u64,
    pub persona_name: Option<String>,
    pub most_recent: bool,
    pub timestamp: u64,
}

/// Accounts that signed in on this PC (`config\loginusers.vdf`), most recent first.
pub fn login_users(root: &Path) -> Vec<SteamAccount> {
    match read_vdf(&root.join("config").join("loginusers.vdf")) {
        Ok(doc) => parse_login_users(&doc),
        Err(e) => {
            tracing::info!(error = %e, "no readable loginusers.vdf");
            Vec::new()
        }
    }
}

pub fn parse_login_users(doc: &Vdf) -> Vec<SteamAccount> {
    let Some(users) = doc.get("users") else {
        return Vec::new();
    };
    let mut accounts: Vec<SteamAccount> = users
        .entries()
        .iter()
        .filter_map(|(id, node)| {
            let steam_id = id
                .parse()
                .ok()
                .filter(|id| steamid64_to_account_id(*id).is_some())?;
            Some(SteamAccount {
                steam_id,
                persona_name: node.get_str("PersonaName").map(str::to_string),
                most_recent: node.get_str("MostRecent") == Some("1"),
                timestamp: node.get_u64("Timestamp").unwrap_or(0),
            })
        })
        .collect();
    accounts.sort_by_key(|a| std::cmp::Reverse((a.most_recent, a.timestamp)));
    accounts
}

/// Files Steam caches for the library view (ARCHITECTURE.md § 7).
pub const COVER_FILE: &str = "library_600x900.jpg";
pub const HERO_FILE: &str = "library_hero.jpg";
pub const LOGO_FILE: &str = "logo.png";
pub const HEADER_FILE: &str = "header.jpg";

/// Finds a cached image in `appcache\librarycache\<appid>\`, directly or in a
/// hashed sub-folder one level down.
pub fn find_cached_image(root: &Path, appid: u64, file: &str) -> Option<PathBuf> {
    let dir = root
        .join("appcache")
        .join("librarycache")
        .join(appid.to_string());
    let direct = dir.join(file);
    if direct.is_file() {
        return Some(direct);
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    subdirs
        .into_iter()
        .map(|d| d.join(file))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    const MANIFEST: &str = r#""AppState"
{
	"appid"		"1086940"
	"name"		"Baldur's Gate 3"
	"StateFlags"		"6"
	"installdir"		"Baldurs Gate 3"
	"LastPlayed"		"1781694277"
	"SizeOnDisk"		"155359311885"
}"#;

    #[test]
    fn steamid_conversions() {
        assert_eq!(steamid64_to_account_id(76561198033149506), Some(72883778));
        assert_eq!(steamid64_to_account_id(42), None);
        assert_eq!(
            parse_steamid64(" 76561198033149506 "),
            Some(76561198033149506)
        );
        assert_eq!(parse_steamid64("7656119803314950"), None);
        assert_eq!(parse_steamid64("abc"), None);
    }

    #[test]
    fn normalizes_registry_paths() {
        assert_eq!(
            normalize_registry_path("c:/program files (x86)/steam"),
            PathBuf::from(r"c:\program files (x86)\steam")
        );
    }

    #[test]
    fn picks_properly_cased_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Steam");
        fs::create_dir_all(root.join("steamapps")).unwrap();
        let lower = PathBuf::from(root.to_string_lossy().to_lowercase());
        assert_eq!(
            pick_steam_root(Some(lower), vec![root.clone()]),
            Some(root.clone())
        );
        assert_eq!(
            pick_steam_root(None, vec![dir.path().join("nope"), root.clone()]),
            Some(root)
        );
        assert_eq!(pick_steam_root(None, vec![dir.path().join("nope")]), None);
    }

    #[test]
    fn parses_manifest() {
        let lib = Path::new(r"E:\SteamLibrary");
        let m = parse_manifest(MANIFEST, lib).unwrap().unwrap();
        assert_eq!(m.appid, 1086940);
        assert_eq!(m.name, "Baldur's Gate 3");
        assert_eq!(
            m.install_path,
            lib.join("steamapps").join("common").join("Baldurs Gate 3")
        );
        assert_eq!(m.size_bytes, Some(155359311885));
        assert_eq!(m.last_played, Some(1781694277));
        assert!(m.fully_installed);
        assert_eq!(parse_manifest(r#""Other" { }"#, lib).unwrap(), None);
    }

    #[test]
    fn excludes_steam_tools() {
        assert!(is_steam_tool(228980, "Steamworks Common Redistributables"));
        assert!(is_steam_tool(1, "Proton 9.0"));
        assert!(is_steam_tool(1, "Steam Linux Runtime 3.0 (sniper)"));
        assert!(!is_steam_tool(1086940, "Baldur's Gate 3"));
        assert!(!is_steam_tool(2, "Protonaut"));
    }

    #[test]
    fn reads_libraries_and_installed_apps() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Steam");
        let second = dir.path().join("SteamLibrary");
        let unplugged = dir.path().join("Unplugged");
        write(
            &root.join("steamapps").join("libraryfolders.vdf"),
            &format!(
                "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} \"2\" {{ \"path\" \"{}\" }} }}",
                root.display().to_string().replace('\\', "\\\\"),
                second.display().to_string().replace('\\', "\\\\"),
                unplugged.display().to_string().replace('\\', "\\\\"),
            ),
        );
        write(
            &second.join("steamapps").join("appmanifest_1086940.acf"),
            MANIFEST,
        );
        write(
            &second.join("steamapps").join("appmanifest_228980.acf"),
            r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" }"#,
        );
        write(
            &second.join("steamapps").join("appmanifest_1.acf"),
            "\"AppState\" { broken",
        );
        write(&second.join("steamapps").join("notes.txt"), "ignored");

        let (libs, missing) = library_folders(&root);
        assert_eq!(libs, vec![root.clone(), second.clone()]);
        assert_eq!(missing, vec![unplugged]);

        let apps = installed_apps(&second);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].appid, 1086940);
        assert!(installed_apps(&root).is_empty());
    }

    fn m(appid: u64, lib: &str, full: bool, updated: Option<u64>) -> AppManifest {
        AppManifest {
            appid,
            name: "X".into(),
            install_path: PathBuf::from(lib)
                .join("steamapps")
                .join("common")
                .join("X"),
            library: PathBuf::from(lib),
            size_bytes: None,
            last_played: None,
            last_updated: updated,
            fully_installed: full,
        }
    }

    #[test]
    fn duplicate_manifests_prefer_steams_library_then_state_then_date() {
        // Real case: Satisfactory listed under C: with a stale copy on E:.
        let doc = vdf::parse(
            r#""libraryfolders" {
                "0" { "path" "C:\\Steam" "apps" { "526870" "1" } }
                "1" { "path" "E:\\SteamLibrary" "apps" { "1" "1" } }
            }"#,
        )
        .unwrap();
        let owners = parse_library_app_owners(&doc);
        assert_eq!(owners[&526870], PathBuf::from(r"C:\Steam"));

        let list = dedupe_manifests(
            vec![
                m(526870, r"C:\Steam", true, Some(10)),
                m(526870, r"E:\SteamLibrary", true, Some(99)),
                // Not in any "apps" block: fully installed wins, then newest.
                m(2, r"C:\Steam", false, Some(50)),
                m(2, r"E:\SteamLibrary", true, None),
                m(3, r"C:\Steam", true, Some(1)),
                m(3, r"E:\SteamLibrary", true, Some(2)),
            ],
            &owners,
        );
        let libs: Vec<_> = list
            .iter()
            .map(|x| (x.appid, x.library.display().to_string()))
            .collect();
        assert_eq!(
            libs,
            vec![
                (2, r"E:\SteamLibrary".to_string()),
                (3, r"E:\SteamLibrary".to_string()),
                (526870, r"C:\Steam".to_string()),
            ]
        );
    }

    #[test]
    fn library_folders_without_vdf_keeps_root() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("steamapps")).unwrap();
        let (libs, missing) = library_folders(dir.path());
        assert_eq!(libs, vec![dir.path().to_path_buf()]);
        assert!(missing.is_empty());
    }

    #[test]
    fn parses_local_playtime() {
        let doc = vdf::parse(
            r#""UserLocalConfigStore" { "Software" { "Valve" { "Steam" { "apps" {
                "218" { "LastPlayed" "1630508773" "Playtime" "74" }
                "220" { "LastPlayed" "1425734872" }
                "7" { "cloud" { } }
                "notanid" { "Playtime" "5" }
            } } } } }"#,
        )
        .unwrap();
        let map = parse_local_playtime(&doc);
        assert_eq!(map.len(), 2);
        assert_eq!(
            map[&218],
            LocalPlaytime {
                minutes: Some(74),
                last_played: Some(1630508773)
            }
        );
        assert_eq!(map[&220].minutes, None);
    }

    #[test]
    fn parses_login_users_most_recent_first() {
        let doc = vdf::parse(
            r#""users" {
                "76561198000000001" { "PersonaName" "Old" "Timestamp" "100" }
                "76561198033149506" { "PersonaName" "Me" "MostRecent" "1" "Timestamp" "50" }
                "123" { "PersonaName" "Invalid" }
            }"#,
        )
        .unwrap();
        let users = parse_login_users(&doc);
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].steam_id, 76561198033149506);
        assert_eq!(users[0].persona_name.as_deref(), Some("Me"));
        assert_eq!(users[1].steam_id, 76561198000000001);
    }

    #[test]
    fn finds_cached_images_in_hashed_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("appcache").join("librarycache");
        write(&cache.join("10").join(COVER_FILE), "x");
        write(&cache.join("20").join("48a2fcbd").join(HEADER_FILE), "x");
        assert_eq!(
            find_cached_image(dir.path(), 10, COVER_FILE),
            Some(cache.join("10").join(COVER_FILE))
        );
        assert_eq!(
            find_cached_image(dir.path(), 20, HEADER_FILE),
            Some(cache.join("20").join("48a2fcbd").join(HEADER_FILE))
        );
        assert_eq!(find_cached_image(dir.path(), 20, COVER_FILE), None);
        assert_eq!(find_cached_image(dir.path(), 30, COVER_FILE), None);
    }
}
