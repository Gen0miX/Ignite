//! Goldberg (gbe_fork) options edited from Ignite: the user's profile, the
//! in-game overlay, connectivity/DLCs and compatibility workarounds.
//!
//! Goldberg reads `configs.{user,main,app,overlay}.ini` from its global
//! folder (`%APPDATA%\GSE Saves\settings`) and from the game's
//! `steam_settings` folder; the game's files win, key by key. Ignite edits
//! only the keys listed in [`OPTIONS`] (checked against gbe_fork's
//! `steam_settings.EXAMPLE/configs.*.EXAMPLE.ini`) and keeps everything else
//! in the files: comments, unknown keys and their order.
//!
//! `local_save_path` and `saves_folder_name` are deliberately not offered:
//! they move the saves Ignite reads achievements from.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFile {
    User,
    Main,
    App,
    Overlay,
}

impl ConfigFile {
    pub fn file_name(self) -> &'static str {
        match self {
            ConfigFile::User => "configs.user.ini",
            ConfigFile::Main => "configs.main.ini",
            ConfigFile::App => "configs.app.ini",
            ConfigFile::Overlay => "configs.overlay.ini",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// `0` / `1`.
    Bool,
    /// Free text up to `max` characters (no line breaks).
    Text { max: usize },
    /// One of these exact values.
    Choice(&'static [&'static str]),
    /// Decimal number within the range.
    Number { min: f64, max: f64 },
}

#[derive(Debug, Clone, Copy)]
pub struct OptionDef {
    /// Stable id shared with the frontend.
    pub id: &'static str,
    pub file: ConfigFile,
    pub section: &'static str,
    pub key: &'static str,
    pub kind: Kind,
    /// Goldberg's own default when no file sets the key.
    pub default: &'static str,
}

/// Steam API language codes (partner.steamgames.com › Localization).
pub const LANGUAGES: &[&str] = &[
    "english",
    "french",
    "german",
    "spanish",
    "latam",
    "italian",
    "portuguese",
    "brazilian",
    "polish",
    "russian",
    "ukrainian",
    "turkish",
    "dutch",
    "swedish",
    "danish",
    "norwegian",
    "finnish",
    "czech",
    "hungarian",
    "romanian",
    "greek",
    "bulgarian",
    "japanese",
    "koreana",
    "schinese",
    "tchinese",
    "thai",
    "vietnamese",
    "indonesian",
    "arabic",
];

const POSITIONS: &[&str] = &[
    "top_left",
    "top_center",
    "top_right",
    "bot_left",
    "bot_center",
    "bot_right",
];
const HARDWARE: &[&str] = &["0", "1", "2", "3"];
const CONTROLLERS: &[&str] = &["XBOX360", "XBOXONE", "PS3", "PS4", "PS5", "SWITCH"];

pub const OVERLAY_ID: &str = "overlay";

pub const OPTIONS: &[OptionDef] = &[
    // Profile.
    OptionDef {
        id: "accountName",
        file: ConfigFile::User,
        section: "user::general",
        key: "account_name",
        kind: Kind::Text { max: 32 },
        default: "gse orca",
    },
    OptionDef {
        id: "language",
        file: ConfigFile::User,
        section: "user::general",
        key: "language",
        kind: Kind::Choice(LANGUAGES),
        default: "english",
    },
    OptionDef {
        id: "ipCountry",
        file: ConfigFile::User,
        section: "user::general",
        key: "ip_country",
        kind: Kind::Text { max: 2 },
        default: "US",
    },
    // In-game overlay.
    OptionDef {
        id: OVERLAY_ID,
        file: ConfigFile::Overlay,
        section: "overlay::general",
        key: "enable_experimental_overlay",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "overlayPosition",
        file: ConfigFile::Overlay,
        section: "overlay::appearance",
        key: "PosAchievement",
        kind: Kind::Choice(POSITIONS),
        default: "bot_right",
    },
    OptionDef {
        id: "overlayDuration",
        file: ConfigFile::Overlay,
        section: "overlay::appearance",
        key: "Notification_Duration_Achievement",
        kind: Kind::Number {
            min: 1.0,
            max: 30.0,
        },
        default: "7.0",
    },
    OptionDef {
        id: "overlayHotkey",
        file: ConfigFile::Overlay,
        section: "overlay::hotkeys",
        key: "key_combo",
        kind: Kind::Text { max: 40 },
        default: "shift + tab",
    },
    OptionDef {
        id: "overlayFps",
        file: ConfigFile::Overlay,
        section: "overlay::general",
        key: "overlay_always_show_fps",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "overlayNoProgress",
        file: ConfigFile::Overlay,
        section: "overlay::general",
        key: "disable_achievement_progress",
        kind: Kind::Bool,
        default: "1",
    },
    OptionDef {
        id: "overlayNoAchievements",
        file: ConfigFile::Overlay,
        section: "overlay::general",
        key: "disable_achievement_notification",
        kind: Kind::Bool,
        default: "0",
    },
    // Connectivity and DLCs.
    OptionDef {
        id: "offline",
        file: ConfigFile::Main,
        section: "main::connectivity",
        key: "offline",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "disableNetworking",
        file: ConfigFile::Main,
        section: "main::connectivity",
        key: "disable_networking",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "unlockAllDlcs",
        file: ConfigFile::App,
        section: "app::dlcs",
        key: "unlock_all",
        kind: Kind::Bool,
        default: "1",
    },
    OptionDef {
        id: "steamHardware",
        file: ConfigFile::Main,
        section: "main::general",
        key: "steam_hardware_type",
        kind: Kind::Choice(HARDWARE),
        default: "0",
    },
    // Compatibility.
    OptionDef {
        id: "achievementsBypass",
        file: ConfigFile::Main,
        section: "main::misc",
        key: "achievements_bypass",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "newAppTicket",
        file: ConfigFile::Main,
        section: "main::general",
        key: "new_app_ticket",
        kind: Kind::Bool,
        default: "1",
    },
    OptionDef {
        id: "steamInput",
        file: ConfigFile::App,
        section: "app::controller",
        key: "steam_input",
        kind: Kind::Bool,
        default: "0",
    },
    OptionDef {
        id: "controllerType",
        file: ConfigFile::App,
        section: "app::controller",
        key: "type",
        kind: Kind::Choice(CONTROLLERS),
        default: "XBOX360",
    },
];

pub fn option(id: &str) -> Option<&'static OptionDef> {
    OPTIONS.iter().find(|o| o.id == id)
}

/// One option as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GoldbergSetting {
    pub id: String,
    /// Value set at this level (`None`: not set here).
    pub value: Option<String>,
    /// Value in effect without it: the global one for a game, Goldberg's
    /// default for the global level.
    pub inherited: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GoldbergSettings {
    /// Folder holding the files edited at this level.
    pub folder: String,
    pub settings: Vec<GoldbergSetting>,
}

/// Goldberg's global settings folder.
pub fn global_dir() -> PathBuf {
    super::goldberg::saves_root().join("settings")
}

// ---------------------------------------------------------------- ini edits

fn section_name(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .map(str::trim)
}

fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with(';') || t.starts_with('[') {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim())
}

/// Value of `key` in `[section]` (the last one wins, like Goldberg).
pub fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    let mut current: Option<&str> = None;
    let mut found = None;
    for line in text.lines() {
        if let Some(name) = section_name(line) {
            current = Some(name);
            continue;
        }
        if current.is_some_and(|s| s.eq_ignore_ascii_case(section))
            && key_of(line).is_some_and(|k| k.eq_ignore_ascii_case(key))
        {
            found = line.split_once('=').map(|(_, v)| v.trim().to_string());
        }
    }
    found
}

/// Sets (`Some`) or removes (`None`) `key` in `[section]`, keeping every
/// other line as is. A missing section is appended at the end.
pub fn ini_set(text: &str, section: &str, key: &str, value: Option<&str>) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 2);
    let mut current: Option<&str> = None;
    let mut written = false;
    // Index in `out` after the last non-blank line of the target section.
    let mut section_end: Option<usize> = None;
    for line in &lines {
        if let Some(name) = section_name(line) {
            current = Some(name);
            out.push((*line).to_string());
            if name.eq_ignore_ascii_case(section) {
                section_end = Some(out.len());
            }
            continue;
        }
        let in_section = current.is_some_and(|s| s.eq_ignore_ascii_case(section));
        if in_section && key_of(line).is_some_and(|k| k.eq_ignore_ascii_case(key)) {
            // First occurrence is replaced, duplicates dropped.
            if let (Some(v), false) = (value, written) {
                out.push(format!("{key}={v}"));
                section_end = Some(out.len());
            }
            written = true;
            continue;
        }
        out.push((*line).to_string());
        if in_section && !line.trim().is_empty() {
            section_end = Some(out.len());
        }
    }
    if let (Some(v), false) = (value, written) {
        match section_end {
            Some(i) => out.insert(i, format!("{key}={v}")),
            None => {
                if out.last().is_some_and(|l| !l.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push(format!("[{section}]"));
                out.push(format!("{key}={v}"));
            }
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// Checks a value typed in the UI and returns it in Goldberg's format.
pub fn validate(def: &OptionDef, value: &str) -> AppResult<String> {
    let v = value.trim();
    let bad = || AppError::Invalid(format!("invalid value for {}", def.id));
    if v.contains(['\n', '\r']) {
        return Err(bad());
    }
    match def.kind {
        Kind::Bool => match v {
            "1" | "true" => Ok("1".into()),
            "0" | "false" => Ok("0".into()),
            _ => Err(bad()),
        },
        Kind::Text { max } => {
            if v.is_empty() || v.chars().count() > max {
                return Err(bad());
            }
            if def.id == "ipCountry" {
                if v.len() != 2 || !v.chars().all(|c| c.is_ascii_alphabetic()) {
                    return Err(bad());
                }
                return Ok(v.to_ascii_uppercase());
            }
            Ok(v.to_string())
        }
        Kind::Choice(choices) => choices
            .iter()
            .find(|c| c.eq_ignore_ascii_case(v))
            .map(|c| (*c).to_string())
            .ok_or_else(bad),
        Kind::Number { min, max } => {
            let n: f64 = v.replace(',', ".").parse().map_err(|_| bad())?;
            if !(min..=max).contains(&n) {
                return Err(bad());
            }
            Ok(format!("{n:.1}"))
        }
    }
}

// ------------------------------------------------------------- read / write

fn read(dir: &Path, file: ConfigFile) -> Option<String> {
    std::fs::read_to_string(dir.join(file.file_name())).ok()
}

/// Value set for `def` in the files of `dir`.
pub fn value_in(dir: &Path, def: &OptionDef) -> Option<String> {
    read(dir, def.file).and_then(|t| ini_get(&t, def.section, def.key))
}

/// Value in effect for a game: its own files, then the global ones, then
/// Goldberg's default.
pub fn effective(game_settings: Option<&Path>, def: &OptionDef) -> String {
    game_settings
        .and_then(|d| value_in(d, def))
        .or_else(|| value_in(&global_dir(), def))
        .unwrap_or_else(|| def.default.to_string())
}

/// Every option at one level: `game_settings = None` is the global level.
pub fn view(game_settings: Option<&Path>) -> GoldbergSettings {
    let global = global_dir();
    let folder = game_settings.unwrap_or(&global);
    let settings = OPTIONS
        .iter()
        .map(|def| GoldbergSetting {
            id: def.id.to_string(),
            value: value_in(folder, def),
            inherited: match game_settings {
                Some(_) => value_in(&global, def).unwrap_or_else(|| def.default.to_string()),
                None => def.default.to_string(),
            },
        })
        .collect();
    GoldbergSettings {
        folder: folder.display().to_string(),
        settings,
    }
}

/// Sets or clears (`None`: back to the inherited value) one option in the
/// files of `folder`. Turning the overlay on in a folder without overlay
/// config writes Ignite's themed template.
pub fn set(folder: &Path, id: &str, value: Option<&str>, overlay_template: &str) -> AppResult<()> {
    let def = option(id).ok_or_else(|| AppError::Invalid(format!("unknown option {id}")))?;
    let value = value.map(|v| validate(def, v)).transpose()?;
    let path = folder.join(def.file.file_name());
    let existing = std::fs::read_to_string(&path).ok();
    let base = match (&existing, def.id, value.as_deref()) {
        (None, OVERLAY_ID, Some("1")) => overlay_template.to_string(),
        (None, _, None) => return Ok(()),
        (None, _, _) => String::new(),
        (Some(text), _, _) => text.clone(),
    };
    let text = ini_set(&base, def.section, def.key, value.as_deref());
    std::fs::create_dir_all(folder)
        .map_err(|e| AppError::Invalid(format!("cannot create {}: {e}", folder.display())))?;
    std::fs::write(&path, text)
        .map_err(|e| AppError::Invalid(format!("cannot write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_last_value_of_the_right_section() {
        let ini = "[a]\nkey=1\n# key=9\n[b]\nkey=2\n[a]\nKEY = 3\n";
        assert_eq!(ini_get(ini, "a", "key").as_deref(), Some("3"));
        assert_eq!(ini_get(ini, "b", "key").as_deref(), Some("2"));
        assert_eq!(ini_get(ini, "c", "key"), None);
    }

    #[test]
    fn replaces_in_place_and_keeps_the_rest() {
        let ini = "# header\n[user::general]\n# name\naccount_name=old\nlanguage=english\n\n[user::saves]\nx=1\n";
        let out = ini_set(ini, "user::general", "account_name", Some("Gen0miX"));
        assert_eq!(
            out,
            "# header\n[user::general]\n# name\naccount_name=Gen0miX\nlanguage=english\n\n[user::saves]\nx=1\n"
        );
    }

    #[test]
    fn adds_to_an_existing_section_before_the_blank_line() {
        let ini = "[user::general]\nlanguage=english\n\n[user::saves]\nx=1\n";
        let out = ini_set(ini, "user::general", "ip_country", Some("CH"));
        assert_eq!(
            out,
            "[user::general]\nlanguage=english\nip_country=CH\n\n[user::saves]\nx=1\n"
        );
    }

    #[test]
    fn appends_a_missing_section_and_removes_keys() {
        let out = ini_set("[a]\nx=1\n", "main::misc", "achievements_bypass", Some("1"));
        assert_eq!(out, "[a]\nx=1\n\n[main::misc]\nachievements_bypass=1\n");
        let out = ini_set(&out, "main::misc", "achievements_bypass", None);
        assert_eq!(out, "[a]\nx=1\n\n[main::misc]\n");
        assert_eq!(ini_set("", "s", "k", Some("v")), "[s]\nk=v\n");
    }

    #[test]
    fn duplicates_are_collapsed() {
        let out = ini_set("[s]\nk=1\nk=2\n", "s", "k", Some("3"));
        assert_eq!(out, "[s]\nk=3\n");
    }

    #[test]
    fn values_are_validated() {
        let get = |id| option(id).unwrap();
        assert_eq!(validate(get("offline"), "true").unwrap(), "1");
        assert!(validate(get("offline"), "yes").is_err());
        assert_eq!(validate(get("language"), "French").unwrap(), "french");
        assert!(validate(get("language"), "klingon").is_err());
        assert_eq!(validate(get("ipCountry"), "ch").unwrap(), "CH");
        assert!(validate(get("ipCountry"), "CHE").is_err());
        assert_eq!(validate(get("overlayDuration"), "6,5").unwrap(), "6.5");
        assert!(validate(get("overlayDuration"), "90").is_err());
        assert!(validate(get("accountName"), "").is_err());
        assert!(validate(get("accountName"), "a\nb").is_err());
    }

    #[test]
    fn ids_are_unique_and_defaults_valid() {
        for (i, a) in OPTIONS.iter().enumerate() {
            assert!(OPTIONS[i + 1..].iter().all(|b| b.id != a.id), "{}", a.id);
            assert!(validate(a, a.default).is_ok(), "{}", a.id);
        }
    }

    #[test]
    fn game_level_overrides_and_resets() {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path().join("steam_settings");
        set(&game, "offline", Some("1"), "").unwrap();
        set(&game, "accountName", Some("Joueur"), "").unwrap();
        assert_eq!(
            value_in(&game, option("offline").unwrap()).as_deref(),
            Some("1")
        );
        let user = std::fs::read_to_string(game.join("configs.user.ini")).unwrap();
        assert_eq!(user, "[user::general]\naccount_name=Joueur\n");
        // Reset: back to the inherited value.
        set(&game, "offline", None, "").unwrap();
        assert_eq!(value_in(&game, option("offline").unwrap()), None);
        // Clearing a key in a missing file creates nothing.
        set(&game, "steamInput", None, "").unwrap();
        assert!(!game.join("configs.app.ini").exists());
        assert!(set(&game, "local_save_path", Some("x"), "").is_err());
    }

    #[test]
    fn overlay_template_only_for_a_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        let template = "# themed\n[overlay::general]\nenable_experimental_overlay=1\n";
        set(tmp.path(), OVERLAY_ID, Some("1"), template).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("configs.overlay.ini")).unwrap();
        assert!(text.starts_with("# themed"));
        set(tmp.path(), OVERLAY_ID, Some("0"), template).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("configs.overlay.ini")).unwrap();
        assert!(text.starts_with("# themed"));
        assert!(text.contains("enable_experimental_overlay=0"));
    }
}
