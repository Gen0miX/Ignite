//! Goldberg Steam Emulator for games added by hand (`local` sources with an
//! executable).
//!
//! Installing replaces the game's `steam_api(64).dll` with Goldberg's, backs
//! the original up and writes the `steam_settings` folder (appid, achievement
//! definitions generated from the Steam Web API). Progress is then read from
//! `%APPDATA%\GSE saves\<appid>\achievements.json` and announced by an
//! Ignite popup (or Goldberg's in-game overlay when turned on).
//!
//! Nothing here applies to launchers, Steam games or emulators: only a
//! `local` source with a real `.exe`.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::repo::achievements::{self, AchievementRow};
use crate::db::repo::{game_sources, games, settings};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::http;
use crate::models::SourceKind;
use crate::popup::{self, Popup, PopupKind};
use crate::sources::steam::achievements as steam;
use crate::sources::unix_to_rfc3339;

/// Setting: announce new achievements with an Ignite popup.
pub const NOTIFY_KEY: &str = "achievements.toasts";
pub const ACHIEVEMENT_UNLOCKED_EVENT: &str = "achievement-unlocked";

const RELEASES_API: &str = "https://api.github.com/repos/Detanup01/gbe_fork/releases/latest";
const ARCHIVE_PREFIX: &str = "release/experimental/";
const BACKUP_SUFFIX: &str = ".ignite-original";
/// Goldberg's DLLs are around 1.5 MB; Valve's are a few hundred KB.
const GOLDBERG_MIN_BYTES: u64 = 900_000;

#[cfg(windows)]
pub fn saves_root() -> PathBuf {
    // Same folder Goldberg and achievement-watchdog use.
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GSE saves")
}

#[cfg(not(windows))]
pub fn saves_root() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GSE saves")
}

fn saves_file(appid: u32) -> PathBuf {
    saves_root()
        .join(appid.to_string())
        .join("achievements.json")
}

/// Classic Goldberg location, kept for games migrated from the original
/// emulator.
fn classic_saves_file(appid: u32) -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from).map(|root| {
        root.join("Goldberg SteamEmu Saves")
            .join(appid.to_string())
            .join("achievements.json")
    })
}

/// `%PUBLIC%\Documents\Steam`: shared saves some Goldberg builds use.
fn public_steam_root() -> Option<PathBuf> {
    let public = std::env::var_os("PUBLIC").map(PathBuf::from).or_else(|| {
        std::env::var_os("SystemDrive")
            .map(PathBuf::from)
            .map(|d| d.join("Users").join("Public"))
    })?;
    Some(public.join("Documents").join("Steam"))
}

/// Cap on the Public Documents walk: a huge tree must not block for long.
const MAX_FIND_DEPTH: usize = 6;
const MAX_FIND_ENTRIES: usize = 4000;

/// Steam emulators save unlocks under different names: Goldberg/GBE use
/// `achievements.json`, other Steam emulators use `achievements.ini`.
fn is_achievements_file(name: &std::ffi::OsStr) -> bool {
    name == "achievements.json" || name == "achievements.ini"
}

/// Depth-limited walk of `root` looking for `<appid>\achievements.*`.
fn walk_for_appid(root: &Path, appid: u32) -> Vec<PathBuf> {
    let target = appid.to_string();
    let mut found = Vec::new();
    let mut visited = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if visited >= MAX_FIND_ENTRIES {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            visited += 1;
            if visited >= MAX_FIND_ENTRIES {
                break;
            }
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if depth < MAX_FIND_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if path.file_name().is_some_and(is_achievements_file)
                && path
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|n| n == target.as_str())
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Every existing `achievements.json` for `appid` in the usual locations.
pub fn find_save_files(appid: u32) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files.push(saves_file(appid));
    if let Some(path) = classic_saves_file(appid) {
        files.push(path);
    }
    if let Some(root) = public_steam_root() {
        files.extend(walk_for_appid(&root, appid));
    }
    files.sort();
    files.dedup();
    files.retain(|p| p.is_file());
    files
}

/// Result of looking for achievements already unlocked on this machine.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GoldbergFind {
    /// Achievement saves files found (absolute paths).
    pub paths: Vec<String>,
    /// Achievement api names already unlocked (deduplicated, sorted).
    pub unlocked: Vec<String>,
}

/// Looks for achievements already unlocked for `appid` in the usual Goldberg
/// save locations, including `%PUBLIC%\Documents\Steam`.
pub fn find_saves(appid: u32) -> GoldbergFind {
    let paths: Vec<String> = find_save_files(appid)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();
    let mut unlocked: Vec<String> = read_earned_for(appid).into_keys().collect();
    unlocked.sort();
    GoldbergFind { paths, unlocked }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Architecture {
    X64,
    X86,
}

impl Architecture {
    fn dll_name(self) -> &'static str {
        match self {
            Architecture::X64 => "steam_api64.dll",
            Architecture::X86 => "steam_api.dll",
        }
    }

    fn archive_dir(self) -> &'static str {
        match self {
            Architecture::X64 => "x64",
            Architecture::X86 => "x86",
        }
    }
}

/// What the UI has to fix for achievements to work.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum GoldbergIssue {
    /// Not a local game with an executable.
    NoExecutable,
    /// No `steam_api(64).dll` found (needs a Steamworks game).
    NoDll,
    /// The DLL is still Valve's.
    NotInstalled,
    /// No Steam appid configured.
    NeedsAppid,
    /// A Steam Web API key is needed to build the definitions.
    NeedsSteamKey,
    /// `steam_settings\achievements.json` is missing.
    NoDefinitions,
    /// No achievement earned yet: Goldberg only creates the saves file then.
    NoSaves,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GoldbergStatus {
    pub game_id: String,
    pub source_id: String,
    pub game_title: String,
    pub exe_path: Option<String>,
    pub game_dir: Option<String>,
    pub dll_path: Option<String>,
    pub architecture: Option<Architecture>,
    pub installed: bool,
    pub original_backed_up: bool,
    pub appid: Option<u32>,
    pub settings_dir: Option<String>,
    pub definitions_ready: bool,
    pub saves_path: Option<String>,
    pub saves_ready: bool,
    /// Goldberg's in-game overlay is turned on (`configs.overlay.ini`).
    pub overlay: bool,
    pub issues: Vec<GoldbergIssue>,
}

impl GoldbergStatus {
    /// Installed and configured. `NoSaves` is not an issue here: Goldberg only
    /// writes the progress file after a first achievement.
    pub fn ready(&self) -> bool {
        self.installed && self.appid.is_some() && self.definitions_ready
    }
}

pub fn is_eligible_source(source: &crate::db::repo::game_sources::GameSource) -> bool {
    source.kind == SourceKind::Local
        && source
            .exe_path
            .as_deref()
            .is_some_and(|e| Path::new(e).is_file())
}

fn game_dir(source: &crate::db::repo::game_sources::GameSource) -> Option<PathBuf> {
    source
        .exe_path
        .as_deref()
        .and_then(|e| Path::new(e).parent())
        .map(Path::to_path_buf)
        .filter(|d| d.is_dir())
}

/// DLL name already present in the install folder (depth 2, deterministic).
fn find_dll(dir: &Path) -> Option<PathBuf> {
    let names = ["steam_api64.dll", "steam_api.dll"];
    for name in names {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name() != Some("steam_settings".as_ref()))
        .collect();
    subdirs.sort();
    for sub in subdirs {
        for name in names {
            let candidate = sub.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_goldberg_dll(dll: &Path) -> bool {
    if Path::new(&format!("{}{BACKUP_SUFFIX}", dll.display())).is_file() {
        return true;
    }
    if dll
        .metadata()
        .map(|m| m.len() >= GOLDBERG_MIN_BYTES)
        .unwrap_or(false)
    {
        return true;
    }
    crate::sources::local::version_strings(dll).is_some_and(|(product, description)| {
        [product, description]
            .into_iter()
            .flatten()
            .any(|s| s.to_lowercase().contains("goldberg"))
    })
}

/// Machine type from the PE header of the game executable.
pub fn pe_architecture(exe: &Path) -> Option<Architecture> {
    let data = std::fs::read(exe).ok()?;
    if data.len() < 0x40 || &data[0..2] != b"MZ" {
        return None;
    }
    let offset = u32::from_le_bytes(data.get(0x3c..0x40)?.try_into().ok()?) as usize;
    let machine = data.get(offset + 4..offset + 6)?;
    match u16::from_le_bytes(machine.try_into().ok()?) {
        0x014c => Some(Architecture::X86),
        _ => None,
    }
}

/// Architecture of the executable, else of the existing DLL.
fn architecture_for(exe: &Path, dll: Option<&Path>) -> Architecture {
    if let Some(a) = pe_architecture(exe) {
        return a;
    }
    if dll.is_some_and(|d| d.file_name().is_some_and(|n| n == "steam_api.dll")) {
        return Architecture::X86;
    }
    Architecture::X64
}

fn read_appid_file(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
}

/// Appid from the database, then from the usual `steam_appid.txt` places.
fn find_appid(
    source: &crate::db::repo::game_sources::GameSource,
    dll: Option<&Path>,
    dir: &Path,
) -> Option<u32> {
    source
        .goldberg_appid
        .and_then(|id| u32::try_from(id).ok())
        .or_else(|| {
            dll.and_then(|d| {
                read_appid_file(&d.parent()?.join("steam_settings").join("steam_appid.txt"))
            })
        })
        .or_else(|| read_appid_file(&dir.join("steam_settings").join("steam_appid.txt")))
        .or_else(|| read_appid_file(&dir.join("steam_appid.txt")))
}

fn settings_dir(dll: Option<&Path>, dir: &Path) -> PathBuf {
    dll.and_then(|d| d.parent())
        .unwrap_or(dir)
        .join("steam_settings")
}

/// Where this game's `steam_settings` folder lives (beside the DLL).
pub fn settings_dir_for(dir: &Path) -> PathBuf {
    settings_dir(find_dll(dir).as_deref(), dir)
}

/// Everything the UI shows: cheap, no network.
pub fn status(
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
) -> GoldbergStatus {
    let title = games::get(conn, &source.game_id)
        .map(|g| g.title)
        .unwrap_or_default();
    let mut issues = Vec::new();
    let dir = game_dir(source);
    if !is_eligible_source(source) || dir.is_none() {
        issues.push(GoldbergIssue::NoExecutable);
    }
    let dir = dir.unwrap_or_default();
    let dll = dir.is_dir().then(|| find_dll(&dir)).flatten();
    if dll.is_none() && dir.is_dir() {
        issues.push(GoldbergIssue::NoDll);
    }
    let installed = dll.as_deref().is_some_and(is_goldberg_dll);
    if dll.is_some() && !installed {
        issues.push(GoldbergIssue::NotInstalled);
    }
    let appid = find_appid(source, dll.as_deref(), &dir);
    if appid.is_none() {
        issues.push(GoldbergIssue::NeedsAppid);
    }
    let settings = settings_dir(dll.as_deref(), &dir);
    let definitions = settings.join("achievements.json").is_file();
    if !definitions {
        issues.push(GoldbergIssue::NoDefinitions);
    }
    let saves_path = appid.map(saves_file);
    let saves_ready = saves_path.as_deref().is_some_and(Path::is_file);
    if appid.is_some() && !saves_ready {
        issues.push(GoldbergIssue::NoSaves);
    }
    GoldbergStatus {
        game_id: source.game_id.clone(),
        source_id: source.id.clone(),
        game_title: title,
        exe_path: source.exe_path.clone(),
        game_dir: (!dir.as_os_str().is_empty()).then(|| dir.display().to_string()),
        dll_path: dll
            .as_ref()
            .and_then(|d| d.parent().map(|p| p.display().to_string())),
        architecture: Some(architecture_for(
            Path::new(source.exe_path.as_deref().unwrap_or("")),
            dll.as_deref(),
        )),
        installed,
        original_backed_up: dll
            .as_deref()
            .is_some_and(|d| Path::new(&format!("{}{BACKUP_SUFFIX}", d.display())).is_file()),
        appid,
        settings_dir: (!settings.as_os_str().is_empty()).then(|| settings.display().to_string()),
        definitions_ready: definitions,
        saves_path: saves_path.as_ref().map(|p| p.display().to_string()),
        saves_ready,
        overlay: std::fs::read_to_string(settings.join(OVERLAY_FILE))
            .is_ok_and(|ini| overlay_on(&ini)),
        issues,
    }
}

#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn pick_asset(release: &Release) -> Option<&Asset> {
    let mut candidates: Vec<&Asset> = release
        .assets
        .iter()
        .filter(|a| a.name.starts_with("emu-win-release") && a.name.ends_with(".7z"))
        .collect();
    // Prefer the vs22 build (works on older Windows too), else any.
    candidates.sort_by_key(|a| !a.name.contains("-vs22"));
    candidates.first().copied()
}

/// Goldberg's release archive is a few tens of MB; ureq defaults to 10 MB.
const MAX_DOWNLOAD: u64 = 128 * 1024 * 1024;

fn download(url: &str) -> AppResult<Vec<u8>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(concat!("Ignite/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| http::network_error(e, None))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .read_to_vec()
        .map_err(|e| http::network_error(e, None))
}

/// Downloads the latest Goldberg release and returns the DLL for `arch`.
pub fn fetch_goldberg_dll(arch: Architecture) -> AppResult<Vec<u8>> {
    let api = download(RELEASES_API)?;
    let release: Release = serde_json::from_slice(&api)?;
    let asset = pick_asset(&release).ok_or_else(|| AppError::Network {
        status: None,
        message: "no Windows release asset on gbe_fork".into(),
    })?;
    let archive = download(&asset.browser_download_url)?;
    let mut reader =
        sevenz_rust2::ArchiveReader::new(Cursor::new(archive), sevenz_rust2::Password::empty())
            .map_err(|e| AppError::Invalid(format!("Goldberg archive: {e}")))?;
    let wanted = format!("{ARCHIVE_PREFIX}{}/{}", arch.archive_dir(), arch.dll_name());
    if let Ok(bytes) = reader.read_file(&wanted) {
        return Ok(bytes);
    }
    // The layout moved between releases: keep the matching file name.
    let suffix = format!("experimental/{}/{}", arch.archive_dir(), arch.dll_name());
    let mut found = None;
    reader
        .for_each_entries(|entry, data| {
            if !entry.is_directory() && entry.name().replace('\\', "/").ends_with(&suffix) {
                let mut bytes = Vec::new();
                data.read_to_end(&mut bytes)?;
                found = Some(bytes);
                return Ok(false);
            }
            Ok(true)
        })
        .map_err(|e| AppError::Invalid(format!("Goldberg archive: {e}")))?;
    found.ok_or_else(|| {
        AppError::Invalid(format!(
            "Goldberg archive: {wanted} not found in the release"
        ))
    })
}

fn backup_and_write(dll: &Path, bytes: &[u8]) -> AppResult<()> {
    if dll.is_file() && !is_goldberg_dll(dll) {
        let backup = PathBuf::from(format!("{}{BACKUP_SUFFIX}", dll.display()));
        if !backup.is_file() {
            std::fs::copy(dll, &backup)
                .map_err(|e| AppError::Invalid(format!("cannot back up {}: {e}", dll.display())))?;
        }
    }
    std::fs::write(dll, bytes)
        .map_err(|e| AppError::Invalid(format!("cannot write {}: {e}", dll.display())))
}

fn write_appid(settings: &Path, appid: u32) -> AppResult<()> {
    std::fs::create_dir_all(settings)
        .map_err(|e| AppError::Invalid(format!("cannot create steam_settings: {e}")))?;
    std::fs::write(settings.join("steam_appid.txt"), appid.to_string())
        .map_err(|e| AppError::Invalid(format!("cannot write steam_appid.txt: {e}")))
}

fn icon_extension(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or("").to_lowercase();
    if path.ends_with(".png") {
        "png"
    } else if path.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    }
}

fn safe_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "achievement".into()
    } else {
        sanitized
    }
}

/// Writes `achievements.json` and its icons from the Steam schema.
fn write_definitions(settings: &Path, key: &str, appid: u32) -> AppResult<usize> {
    let entries = steam::fetch_schema(key, u64::from(appid))?;
    if entries.is_empty() {
        return Ok(0);
    }
    let images = settings.join("images");
    std::fs::create_dir_all(&images)
        .map_err(|e| AppError::Invalid(format!("cannot create images folder: {e}")))?;

    let jobs: Vec<(String, String)> = entries
        .iter()
        .filter_map(|e| e.icon.clone())
        .map(|url| {
            (
                safe_name(&format!(
                    "{}.{}",
                    url_achievement_name(&url),
                    icon_extension(&url)
                )),
                url,
            )
        })
        .collect();
    let missing: Vec<&(String, String)> = jobs
        .iter()
        .filter(|(f, _)| !images.join(f).is_file())
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..6.min(missing.len()) {
            scope.spawn(|| {
                use std::sync::atomic::Ordering;
                while let Some((file, url)) = missing.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Ok(bytes) = download(url) {
                        let _ = std::fs::write(images.join(file), bytes);
                    }
                }
            });
        }
    });

    let list: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let mut icon = serde_json::Map::new();
            if let Some(url) = &e.icon {
                let file = format!(
                    "images/{}",
                    safe_name(&format!(
                        "{}.{}",
                        url_achievement_name(url),
                        icon_extension(url)
                    ))
                );
                icon.insert("icon".into(), file.clone().into());
                icon.insert("icongray".into(), file.into());
            }
            let mut obj = serde_json::Map::new();
            obj.insert(
                "description".into(),
                e.description.clone().unwrap_or_default().into(),
            );
            obj.insert(
                "displayName".into(),
                e.name.clone().unwrap_or_else(|| e.api_name.clone()).into(),
            );
            obj.insert(
                "hidden".into(),
                if e.hidden { "1".into() } else { "0".into() },
            );
            obj.insert("name".into(), e.api_name.clone().into());
            obj.extend(icon);
            serde_json::Value::Object(obj)
        })
        .collect();
    let json = serde_json::to_string_pretty(&list)?;
    std::fs::write(settings.join("achievements.json"), json)
        .map_err(|e| AppError::Invalid(format!("cannot write achievements.json: {e}")))?;
    Ok(entries.len())
}

/// The Steam schema gives no api name in the icon URL; the file name only has
/// to be stable per icon, so keep the URL's last path segment.
fn url_achievement_name(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("achievement")
        .split('.')
        .next()
        .unwrap_or("achievement")
        .to_string()
}

/// Downloads, installs and configures Goldberg. `appid` overrides the stored
/// one; `force` reinstalls the DLL even when Goldberg is already there.
pub fn install(
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
    appid: Option<u32>,
    force: bool,
) -> AppResult<GoldbergStatus> {
    if !is_eligible_source(source) {
        return Err(AppError::Invalid(
            "only a local game with an executable is supported".into(),
        ));
    }
    let dir = game_dir(source).ok_or_else(|| AppError::Invalid("game folder not found".into()))?;
    let existing = find_dll(&dir);
    let arch = architecture_for(
        Path::new(source.exe_path.as_deref().unwrap_or("")),
        existing.as_deref(),
    );
    let dll = existing.unwrap_or_else(|| dir.join(arch.dll_name()));

    if force || !is_goldberg_dll(&dll) {
        let bytes = fetch_goldberg_dll(arch)?;
        backup_and_write(&dll, &bytes)?;
    }

    let settings = settings_dir(Some(&dll), &dir);
    let appid = appid.or_else(|| find_appid(source, Some(&dll), &dir));
    if let Some(id) = appid {
        game_sources::set_goldberg_appid(conn, &source.id, Some(i64::from(id)))?;
        write_appid(&settings, id)?;
    }
    Ok(status(conn, source))
}

/// Writes the appid and (re)generates the definitions from the Steam schema.
pub fn configure(
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
    appid: u32,
    key: Option<&str>,
) -> AppResult<GoldbergStatus> {
    if !is_eligible_source(source) {
        return Err(AppError::Invalid(
            "only a local game with an executable is supported".into(),
        ));
    }
    let dir = game_dir(source).ok_or_else(|| AppError::Invalid("game folder not found".into()))?;
    let dll = find_dll(&dir).unwrap_or_else(|| dir.join("steam_api64.dll"));
    let settings = settings_dir(Some(&dll), &dir);
    game_sources::set_goldberg_appid(conn, &source.id, Some(i64::from(appid)))?;
    write_appid(&settings, appid)?;
    if let Some(key) = key.filter(|k| !k.is_empty()) {
        write_definitions(&settings, key, appid)?;
    }
    Ok(status(conn, source))
}

#[derive(Debug, Deserialize)]
struct DefinitionRaw {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "displayName")]
    display_name: Option<serde_json::Value>,
    #[serde(default)]
    description: Option<serde_json::Value>,
    #[serde(default)]
    hidden: Option<serde_json::Value>,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub api_name: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub hidden: bool,
}

fn text(value: &Option<serde_json::Value>, language: &str) -> Option<String> {
    match value {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        Some(serde_json::Value::Object(map)) => map
            .get(language)
            .or_else(|| map.get("english"))
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string),
        _ => None,
    }
}

fn truthy(value: &Option<serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(serde_json::Value::String(s)) => matches!(s.as_str(), "1" | "true"),
        _ => false,
    }
}

/// Definitions written in `steam_settings/achievements.json`.
pub fn read_definitions(source: &crate::db::repo::game_sources::GameSource) -> Vec<Definition> {
    let Some(dir) = game_dir(source) else {
        return Vec::new();
    };
    let settings = settings_dir(find_dll(&dir).as_deref(), &dir);
    let Ok(raw) = std::fs::read_to_string(settings.join("achievements.json")) else {
        return Vec::new();
    };
    let Ok(list) = serde_json::from_str::<Vec<DefinitionRaw>>(&raw) else {
        return Vec::new();
    };
    list.into_iter()
        .filter(|d| !d.name.trim().is_empty())
        .map(|d| Definition {
            name: text(&d.display_name, "french").unwrap_or_else(|| d.name.clone()),
            description: text(&d.description, "french"),
            icon: d.icon.map(|icon| {
                if icon.starts_with("http://") || icon.starts_with("https://") {
                    icon
                } else {
                    settings.join(icon).display().to_string()
                }
            }),
            hidden: truthy(&d.hidden),
            api_name: d.name,
        })
        .collect()
}

/// Appid in effect for this source: database first, then the usual
/// `steam_appid.txt` files (a manually prepared copy also works).
fn resolved_appid(source: &crate::db::repo::game_sources::GameSource) -> Option<u32> {
    let dir = game_dir(source)?;
    find_appid(source, find_dll(&dir).as_deref(), &dir)
}

/// `api_name → earned_time` from the Goldberg saves file.
pub fn read_earned(source: &crate::db::repo::game_sources::GameSource) -> HashMap<String, i64> {
    let Some(appid) = resolved_appid(source) else {
        return HashMap::new();
    };
    read_earned_for(appid)
}

/// `api_name → earned_time` merged from every saves file found for `appid`
/// (the current GSE folder, the classic one and Public Documents).
pub fn read_earned_for(appid: u32) -> HashMap<String, i64> {
    merge_earned(&find_save_files(appid))
}

/// Merges several `achievements.*` files; the earliest time wins.
fn merge_earned(files: &[PathBuf]) -> HashMap<String, i64> {
    let mut merged: HashMap<String, i64> = HashMap::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let earned = match file.extension().and_then(|e| e.to_str()) {
            Some("ini") => read_ini_earned(&text),
            _ => read_json_earned(&text),
        };
        for (name, at) in earned {
            merged
                .entry(name)
                .and_modify(|t| *t = (*t).min(at))
                .or_insert(at);
        }
    }
    merged
}

/// Goldberg saves: `{ "<name>": { "earned": bool, "earned_time": unix } }`.
fn read_json_earned(text: &str) -> HashMap<String, i64> {
    let mut earned = HashMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return earned;
    };
    let Some(map) = value.as_object() else {
        return earned;
    };
    for (name, data) in map {
        if data.get("earned").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        let at = data
            .get("earned_time")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        earned.insert(name.clone(), at);
    }
    earned
}

fn truthy_str(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

/// INI saves from other Steam emulators: one `[<api_name>]` section per achievement with
/// `Achieved` and `UnlockTime`, plus a `[SteamAchievements]` index of the
/// unlocked names.
fn read_ini_earned(text: &str) -> HashMap<String, i64> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = Some(name.trim().to_string());
            continue;
        }
        let Some(section) = current.as_deref() else {
            continue;
        };
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section.to_string())
                .or_default()
                .insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let mut unlocked: HashMap<String, i64> = HashMap::new();
    // Every section other than the index is an achievement; `Achieved=1`
    // marks an unlock and `UnlockTime` carries its date.
    for (name, fields) in &sections {
        if name.eq_ignore_ascii_case("SteamAchievements") {
            continue;
        }
        let achieved = fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("achieved"))
            .is_some_and(|(_, v)| truthy_str(v));
        if !achieved {
            continue;
        }
        let at = fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("unlocktime"))
            .and_then(|(_, v)| v.parse::<i64>().ok())
            .unwrap_or(0);
        unlocked
            .entry(name.clone())
            .and_modify(|t| *t = (*t).min(at))
            .or_insert(at);
    }
    // `[SteamAchievements]` maps indexes to unlocked names; it fills in the
    // names whose section is missing (no date known).
    if let Some(index) = sections
        .iter()
        .find(|(s, _)| s.eq_ignore_ascii_case("SteamAchievements"))
    {
        for (key, value) in index.1 {
            if key.chars().all(|c| c.is_ascii_digit()) && !value.is_empty() {
                unlocked.entry(value.clone()).or_insert(0);
            }
        }
    }
    unlocked
}

/// Cache rows for the achievements tab.
pub fn cached_rows(source: &crate::db::repo::game_sources::GameSource) -> Vec<AchievementRow> {
    let earned = read_earned(source);
    read_definitions(source)
        .into_iter()
        .map(|d| AchievementRow {
            unlocked_at: earned
                .get(&d.api_name)
                .copied()
                .and_then(|t| u32::try_from(t.max(0)).ok())
                .and_then(|t| unix_to_rfc3339(u64::from(t))),
            api_name: d.api_name,
            name: Some(d.name),
            description: d.description,
            icon_url: d.icon,
            global_percent: None,
            hidden: d.hidden,
        })
        .collect()
}

/// Stores the definitions/progress of a game in the cache.
pub fn refresh_cache(
    conn: &mut Connection,
    source: &crate::db::repo::game_sources::GameSource,
) -> AppResult<()> {
    let rows = cached_rows(source);
    let list = (!rows.is_empty()).then_some(rows);
    achievements::replace_for_source(conn, &source.id, list.as_deref())
}

fn toasts_enabled(conn: &Connection) -> bool {
    settings::get::<bool>(conn, NOTIFY_KEY)
        .ok()
        .flatten()
        .unwrap_or(true)
}

/// More unlocks than this at once are summed up in a single popup.
const MAX_SEPARATE_POPUPS: usize = 3;

/// Popups for a batch of unlocks: one each, or a summary for a burst (e.g.
/// existing saves found).
fn unlock_popups(game: &str, unlocked: &[Definition]) -> Vec<Popup> {
    if unlocked.len() > MAX_SEPARATE_POPUPS {
        let names: Vec<&str> = unlocked.iter().map(|d| d.name.as_str()).collect();
        return vec![Popup {
            kind: PopupKind::Achievements,
            title: game.to_string(),
            description: Some(names.join(" · ")),
            game: Some(game.to_string()),
            image: unlocked.iter().find_map(|d| d.icon.clone()),
            count: u32::try_from(unlocked.len()).ok(),
        }];
    }
    unlocked
        .iter()
        .map(|d| Popup {
            kind: PopupKind::Achievement,
            title: d.name.clone(),
            description: d.description.clone(),
            game: Some(game.to_string()),
            image: d.icon.clone(),
            count: None,
        })
        .collect()
}

/// Shows a sample popup with one of the game's achievements, so the user
/// can see what an unlock looks like.
pub fn test_popup(
    app: &AppHandle,
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
) -> AppResult<()> {
    let game = games::get(conn, &source.game_id)
        .map(|g| g.title)
        .unwrap_or_default();
    let definitions = read_definitions(source);
    let sample = definitions
        .iter()
        .find(|d| d.icon.is_some() && !d.hidden)
        .or_else(|| definitions.first());
    popup::show(
        app,
        Popup {
            kind: PopupKind::Test,
            title: sample.map(|d| d.name.clone()).unwrap_or_default(),
            description: sample.and_then(|d| d.description.clone()),
            game: Some(game),
            image: sample.and_then(|d| d.icon.clone()),
            count: None,
        },
    )
}

/// Announces unlocked achievements: emits the UI event and, when enabled,
/// shows Ignite popups. `in_game` (the watcher) skips the popups when the
/// game's Goldberg overlay is on: it already shows them inside the game.
pub fn announce_unlocks(
    app: &AppHandle,
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
    unlocked: &[String],
    in_game: bool,
) {
    let definitions = read_definitions(source);
    let title = games::get(conn, &source.game_id)
        .map(|g| g.title)
        .unwrap_or_default();
    let mut shown: Vec<Definition> = Vec::new();
    for api_name in unlocked {
        let definition = definitions.iter().find(|d| d.api_name == *api_name);
        let display = definition
            .map(|d| d.name.clone())
            .unwrap_or_else(|| api_name.clone());
        let _ = app.emit(
            ACHIEVEMENT_UNLOCKED_EVENT,
            serde_json::json!({
                "gameId": source.game_id,
                "apiName": api_name,
                "name": display,
                "description": definition.and_then(|d| d.description.clone()),
            }),
        );
        shown.push(definition.cloned().unwrap_or_else(|| Definition {
            api_name: api_name.clone(),
            name: display,
            description: None,
            icon: None,
            hidden: false,
        }));
    }
    if !toasts_enabled(conn) || (in_game && overlay_enabled_for(source)) {
        return;
    }
    for p in unlock_popups(&title, &shown) {
        if let Err(e) = popup::show(app, p) {
            tracing::warn!(error = %e, "goldberg: popup failed");
        }
    }
}

const OVERLAY_FILE: &str = "configs.overlay.ini";
const OVERLAY_KEY: &str = "enable_experimental_overlay";

/// Goldberg's in-game overlay with Ignite's colors (« Braise »). Keys from
/// gbe_fork's `configs.overlay.EXAMPLE.ini`; colors are 0–1 floats.
const OVERLAY_TEMPLATE: &str = "\
# Written by Ignite: Goldberg's in-game overlay, with Ignite's colors.
# Shift+Tab opens it. Experimental: turn it off in Ignite if the game crashes.
[overlay::general]
enable_experimental_overlay=1
disable_achievement_progress=1

[overlay::appearance]
Font_Size=18.0
Font_Achievement_Title_Bold=1
Icon_Size=64.0
# #241d19 (cards)
Notification_R=0.141
Notification_G=0.114
Notification_B=0.098
Notification_A=0.97
Notification_Rounding=12.0
Notification_Margin_x=12.0
Notification_Margin_y=12.0
Notification_Duration_Achievement=6.0
# #14100e (app background)
Background_R=0.078
Background_G=0.063
Background_B=0.055
Background_A=0.85
# #2a211c (active item)
Element_R=0.165
Element_G=0.129
Element_B=0.110
Element_A=1.0
# #ff7a45 (accent)
ElementHovered_R=1.0
ElementHovered_G=0.478
ElementHovered_B=0.271
ElementHovered_A=1.0
PosAchievement=bot_right
";

fn ini_key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with('#') || line.starts_with(';') {
        return None;
    }
    line.split_once('=').map(|(k, _)| k.trim())
}

/// Whether an overlay config turns the overlay on.
fn overlay_on(ini: &str) -> bool {
    ini.lines()
        .rev()
        .find(|l| ini_key(l) == Some(OVERLAY_KEY))
        .and_then(|l| l.split_once('='))
        .is_some_and(|(_, v)| v.trim() == "1")
}

/// New content of `configs.overlay.ini`: the Ignite template for a new file,
/// otherwise the user's file with only the enable line changed.
fn overlay_ini(existing: Option<&str>, enabled: bool) -> String {
    let value = format!("{OVERLAY_KEY}={}", u8::from(enabled));
    let Some(existing) = existing else {
        return OVERLAY_TEMPLATE.replace(&format!("{OVERLAY_KEY}=1"), &value);
    };
    let mut found = false;
    let mut lines: Vec<String> = existing
        .lines()
        .map(|l| {
            if ini_key(l) == Some(OVERLAY_KEY) {
                found = true;
                value.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !found {
        match lines
            .iter()
            .position(|l| l.trim().eq_ignore_ascii_case("[overlay::general]"))
        {
            Some(i) => lines.insert(i + 1, value),
            None => {
                lines.insert(0, value);
                lines.insert(0, "[overlay::general]".into());
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn overlay_path(source: &crate::db::repo::game_sources::GameSource) -> Option<PathBuf> {
    let dir = game_dir(source)?;
    Some(settings_dir(find_dll(&dir).as_deref(), &dir).join(OVERLAY_FILE))
}

fn overlay_enabled_for(source: &crate::db::repo::game_sources::GameSource) -> bool {
    overlay_path(source)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|ini| overlay_on(&ini))
}

/// Turns Goldberg's in-game overlay on or off (`steam_settings\configs.overlay.ini`).
pub fn set_overlay(
    conn: &Connection,
    source: &crate::db::repo::game_sources::GameSource,
    enabled: bool,
) -> AppResult<GoldbergStatus> {
    if !is_eligible_source(source) {
        return Err(AppError::Invalid(
            "only a local game with an executable is supported".into(),
        ));
    }
    let path =
        overlay_path(source).ok_or_else(|| AppError::Invalid("game folder not found".into()))?;
    let existing = std::fs::read_to_string(&path).ok();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Invalid(format!("cannot create steam_settings: {e}")))?;
    }
    std::fs::write(&path, overlay_ini(existing.as_deref(), enabled))
        .map_err(|e| AppError::Invalid(format!("cannot write {OVERLAY_FILE}: {e}")))?;
    Ok(status(conn, source))
}

/// Achievement watches, seeded on first sight so existing unlocks are silent.
#[derive(Default)]
struct Watcher {
    seen: HashMap<String, HashSet<String>>,
}

impl Watcher {
    /// Returns the achievements earned since the previous tick.
    fn new_unlocks(
        &mut self,
        source_id: &str,
        earned: &HashMap<String, i64>,
    ) -> Option<Vec<String>> {
        let current: HashSet<String> = earned.keys().cloned().collect();
        match self.seen.get_mut(source_id) {
            // First sight after start: seed, do not notify.
            None => {
                self.seen.insert(source_id.to_string(), current);
                None
            }
            Some(seen) => {
                let mut new: Vec<String> = current.difference(seen).cloned().collect();
                new.sort();
                *seen = current;
                Some(new)
            }
        }
    }
}

fn tick(app: &AppHandle) {
    let db = app.state::<Db>();
    let conn = db.conn();
    let sources = match game_sources::list_by_kind(&conn, SourceKind::Local) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "goldberg watcher: cannot list sources");
            return;
        }
    };
    let mut watcher = WATCHER.lock().unwrap_or_else(|e| e.into_inner());
    let live: HashSet<String> = sources.iter().map(|s| s.id.clone()).collect();
    watcher.seen.retain(|id, _| live.contains(id));
    for source in sources
        .iter()
        .filter(|s| is_eligible_source(s) && resolved_appid(s).is_some())
    {
        let earned = read_earned(source);
        let Some(new) = watcher.new_unlocks(&source.id, &earned) else {
            continue;
        };
        if new.is_empty() {
            continue;
        }
        announce_unlocks(app, &conn, source, &new, true);
    }
}

static WATCHER: std::sync::LazyLock<Mutex<Watcher>> =
    std::sync::LazyLock::new(|| Mutex::new(Watcher::default()));

/// Polls the saves files every few seconds while Ignite runs.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(4));
        tick(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(dir: &Path) -> crate::db::repo::game_sources::GameSource {
        crate::db::repo::game_sources::GameSource {
            id: "s".into(),
            game_id: "g".into(),
            kind: SourceKind::Local,
            external_id: None,
            installed: true,
            install_path: None,
            exe_path: Some(dir.join("game.exe").display().to_string()),
            size_bytes: None,
            launch_uri: None,
            process_matchers: None,
            source_playtime_minutes: None,
            source_last_played: None,
            last_synced_at: None,
            goldberg_appid: None,
        }
    }

    #[test]
    fn definitions_and_earned_progress_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path().join("game");
        std::fs::create_dir_all(game.join("steam_settings/images")).unwrap();
        std::fs::write(game.join("game.exe"), b"MZ").unwrap();
        let source = exe(&game);
        std::fs::write(
            game.join("steam_settings/achievements.json"),
            r#"[{"name":"A","displayName":"Premier","description":"D","hidden":"0","icon":"images/a.jpg"}]"#,
        )
        .unwrap();
        let rows = cached_rows(&source);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_deref(), Some("Premier"));
        assert!(
            rows[0]
                .icon_url
                .as_deref()
                .unwrap()
                .ends_with("images\\a.jpg")
                || rows[0]
                    .icon_url
                    .as_deref()
                    .unwrap()
                    .ends_with("images/a.jpg")
        );
        assert!(rows[0].unlocked_at.is_none());

        // No saves file yet.
        assert!(read_earned(&source).is_empty());
    }

    #[test]
    fn hidden_accepts_strings_numbers_and_bools() {
        assert!(truthy(&Some("1".into())));
        assert!(truthy(&Some(1.into())));
        assert!(truthy(&Some(true.into())));
        assert!(!truthy(&Some("0".into())));
        assert!(!truthy(&None));
        let text = Some(serde_json::json!({"french": "Oui", "english": "Yes"}));
        assert_eq!(super::text(&text, "french").as_deref(), Some("Oui"));
    }

    #[test]
    fn arch_detection_and_names() {
        assert_eq!(Architecture::X64.dll_name(), "steam_api64.dll");
        assert_eq!(Architecture::X86.dll_name(), "steam_api.dll");
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("g.exe");
        let mut pe = vec![0u8; 0x40 + 8];
        pe[0..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&(0x40u32).to_le_bytes());
        pe[0x40..0x44].copy_from_slice(b"PE\0\0");
        pe[0x44..0x46].copy_from_slice(&0x014cu16.to_le_bytes());
        std::fs::write(&exe, &pe).unwrap();
        assert_eq!(pe_architecture(&exe), Some(Architecture::X86));
    }

    #[test]
    fn watcher_seeds_then_reports_only_new() {
        let mut w = Watcher::default();
        let first = HashMap::from([("A".to_string(), 10)]);
        assert_eq!(w.new_unlocks("s", &first), None);
        assert_eq!(w.new_unlocks("s", &first), Some(vec![]));
        let mut second = first.clone();
        second.insert("B".into(), 20);
        assert_eq!(w.new_unlocks("s", &second), Some(vec!["B".into()]));
    }

    #[test]
    #[ignore = "network: downloads the latest GitHub release"]
    fn downloads_the_latest_goldberg_dll() {
        let bytes = fetch_goldberg_dll(Architecture::X64).unwrap();
        assert!(bytes.len() > 300_000, "unexpected DLL size");
        assert_eq!(&bytes[0..2], b"MZ");
    }

    #[test]
    fn setup_files_are_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path().join("game");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("game.exe"), b"MZ").unwrap();
        let source = exe(&game);

        write_appid(&game.join("steam_settings"), 42).unwrap();
        assert_eq!(find_appid(&source, None, &game), Some(42));
        let s = status(&Db::open_in_memory().unwrap().conn(), &source);
        assert_eq!(s.appid, Some(42));
        assert!(s.issues.contains(&GoldbergIssue::NoDll));
    }

    #[test]
    fn walk_finds_only_the_matching_appid_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("123").join("achievements.json");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, r#"{"A":{"earned":true,"earned_time":10}}"#).unwrap();
        // Same file name, wrong parent folder: ignored.
        let other = tmp.path().join("456").join("achievements.json");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, r#"{"B":{"earned":true,"earned_time":20}}"#).unwrap();

        assert_eq!(walk_for_appid(tmp.path(), 123), vec![target]);
    }

    #[test]
    fn walk_respects_the_depth_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let deep = tmp.path().join("a/b/c/d/e/f/g/123/achievements.json");
        std::fs::create_dir_all(deep.parent().unwrap()).unwrap();
        std::fs::write(&deep, "{}").unwrap();
        // The <appid> folder sits too deep for the bounded walk.
        assert!(walk_for_appid(tmp.path(), 123).is_empty());
    }

    #[test]
    fn merge_earned_keeps_the_earliest_time() {
        let tmp = tempfile::tempdir().unwrap();
        let one = tmp.path().join("one.json");
        let two = tmp.path().join("two.json");
        std::fs::write(
            &one,
            r#"{"A":{"earned":true,"earned_time":30},"B":{"earned":false}}"#,
        )
        .unwrap();
        std::fs::write(
            &two,
            r#"{"A":{"earned":true,"earned_time":10},"C":{"earned":true}}"#,
        )
        .unwrap();
        let merged = merge_earned(&[one, two]);
        assert_eq!(merged.get("A"), Some(&10));
        assert_eq!(merged.get("C"), Some(&0));
        assert!(!merged.contains_key("B"));
    }

    #[test]
    fn ini_unlocks_are_merged() {
        let text = r#"[ACH_FIRST]
Achieved=1
CurProgress=0
MaxProgress=0
UnlockTime=1700000000
[ACH_SECOND]
Achieved=0
UnlockTime=1700000001
[SteamAchievements]
00000=ACH_FIRST
00001=ACH_THIRD
Count=2
"#;
        let earned = read_ini_earned(text);
        assert_eq!(earned.get("ACH_FIRST"), Some(&1700000000));
        assert_eq!(earned.get("ACH_SECOND"), None);
        assert_eq!(earned.get("ACH_THIRD"), Some(&0));
    }

    #[test]
    fn walk_finds_ini_files() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp
            .path()
            .join("Emu")
            .join("541570")
            .join("achievements.ini");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "[SteamAchievements]\r\nCount=0\r\n").unwrap();
        assert_eq!(walk_for_appid(tmp.path(), 541570), vec![target]);
    }

    #[test]
    fn overlay_template_is_written_on_or_off() {
        let on = overlay_ini(None, true);
        assert!(overlay_on(&on));
        assert!(on.contains("Notification_R=0.141"));
        let off = overlay_ini(None, false);
        assert!(!overlay_on(&off));
        assert!(off.contains("enable_experimental_overlay=0"));
    }

    #[test]
    fn overlay_toggle_keeps_the_user_file() {
        let user = "[overlay::general]\n# comment enable_experimental_overlay=1\nenable_experimental_overlay = 0\nhook_delay_sec=5\n";
        let on = overlay_ini(Some(user), true);
        assert!(overlay_on(&on));
        assert!(on.contains("hook_delay_sec=5"));
        assert!(on.contains("# comment enable_experimental_overlay=1"));
        assert!(!overlay_on(&overlay_ini(Some(&on), false)));
        // Commented-out line only: not on.
        assert!(!overlay_on("#enable_experimental_overlay=1\n"));
    }

    #[test]
    fn overlay_line_added_when_missing() {
        let out = overlay_ini(Some("[overlay::appearance]\nFont_Size=20.0\n"), true);
        assert!(out.starts_with("[overlay::general]\nenable_experimental_overlay=1\n"));
        let out = overlay_ini(Some("[overlay::general]\nhook_delay_sec=0\n"), true);
        assert_eq!(
            out,
            "[overlay::general]\nenable_experimental_overlay=1\nhook_delay_sec=0\n"
        );
    }

    #[test]
    fn set_overlay_writes_beside_the_dll() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("game.exe"), b"MZ").unwrap();
        std::fs::write(tmp.path().join("steam_api64.dll"), b"x").unwrap();
        let db = crate::db::Db::open_in_memory().unwrap();
        let conn = db.conn();
        let source = exe(tmp.path());
        let status = set_overlay(&conn, &source, true).unwrap();
        assert!(status.overlay);
        assert!(overlay_enabled_for(&source));
        let status = set_overlay(&conn, &source, false).unwrap();
        assert!(!status.overlay);
    }

    fn def(name: &str) -> Definition {
        Definition {
            api_name: name.into(),
            name: name.into(),
            description: Some(format!("{name} desc")),
            icon: Some(format!("C:/img/{name}.jpg")),
            hidden: false,
        }
    }

    #[test]
    fn a_few_unlocks_get_one_popup_each() {
        let popups = unlock_popups("Jeu", &[def("A"), def("B")]);
        assert_eq!(popups.len(), 2);
        assert_eq!(popups[0].kind, PopupKind::Achievement);
        assert_eq!(popups[0].title, "A");
        assert_eq!(popups[0].image.as_deref(), Some("C:/img/A.jpg"));
        assert_eq!(popups[1].game.as_deref(), Some("Jeu"));
    }

    #[test]
    fn a_burst_is_summed_up() {
        let defs: Vec<Definition> = ["A", "B", "C", "D"].into_iter().map(def).collect();
        let popups = unlock_popups("Jeu", &defs);
        assert_eq!(popups.len(), 1);
        assert_eq!(popups[0].kind, PopupKind::Achievements);
        assert_eq!(popups[0].count, Some(4));
        assert_eq!(popups[0].description.as_deref(), Some("A · B · C · D"));
    }
}
