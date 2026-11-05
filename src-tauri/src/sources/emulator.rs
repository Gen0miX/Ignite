//! Emulators: each configured emulator turns the ROMs of its folder into
//! games (`game_sources.kind = emulator`, `external_id` = ROM path).

use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::db::repo::settings;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::SourceKind;
use crate::sources::{DetectedGame, ScanResult, Source, SourceStatus};
use crate::tracker::matcher::ProcessMatcher;

pub const SETTINGS_KEY: &str = "emulators";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorConfig {
    pub id: String,
    pub name: String,
    pub exe_path: String,
    /// Arguments; `{rom}` is replaced by the quoted ROM path.
    pub args_template: String,
    pub roms_dir: String,
    /// Extensions without dot, lowercase (`sfc`, `iso`...).
    pub extensions: Vec<String>,
    /// RetroAchievements console id (optional).
    pub console_id: Option<u32>,
}

pub fn load(db: &Db) -> Vec<EmulatorConfig> {
    settings::get(&db.conn(), SETTINGS_KEY)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn validate(c: &EmulatorConfig) -> AppResult<()> {
    if c.name.trim().is_empty() {
        return Err(AppError::Invalid("emulator name is required".into()));
    }
    if !c.exe_path.to_lowercase().ends_with(".exe") || !Path::new(&c.exe_path).is_file() {
        return Err(AppError::Invalid("emulator executable not found".into()));
    }
    if !Path::new(&c.roms_dir).is_dir() {
        return Err(AppError::Invalid("ROM folder not found".into()));
    }
    if c.extensions.is_empty() {
        return Err(AppError::Invalid(
            "at least one extension is required".into(),
        ));
    }
    Ok(())
}

/// `Super Mario World (USA) [!].sfc` → `Super Mario World`.
pub fn clean_rom_title(stem: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for c in stem.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            '_' if depth == 0 => out.push(' '),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        stem.to_string()
    } else {
        t
    }
}

pub fn build_args(template: &str, rom: &str) -> String {
    let quoted = format!("\"{rom}\"");
    if template.contains("{rom}") {
        // Avoid double quotes when the template already quotes the placeholder.
        template
            .replace("\"{rom}\"", &quoted)
            .replace("{rom}", &quoted)
    } else {
        format!("{} {quoted}", template.trim()).trim().to_string()
    }
}

fn walk(dir: &Path, depth: usize, exts: &[String], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 3 {
                walk(&p, depth + 1, exts, out);
            }
        } else if let Some(ext) = p.extension().map(|x| x.to_string_lossy().to_lowercase()) {
            if exts
                .iter()
                .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(&ext))
            {
                out.push(p);
            }
        }
    }
}

pub fn roms(c: &EmulatorConfig) -> Vec<DetectedGame> {
    let mut files = Vec::new();
    walk(Path::new(&c.roms_dir), 0, &c.extensions, &mut files);
    files.sort();
    let matchers = serde_json::to_string(&[ProcessMatcher::ExePath(c.exe_path.clone())]).ok();
    files
        .into_iter()
        .map(|rom| {
            let path = rom.display().to_string();
            DetectedGame {
                external_id: path.to_lowercase(),
                title: clean_rom_title(&rom.file_stem().unwrap_or_default().to_string_lossy()),
                installed: true,
                install_path: Some(c.roms_dir.clone()),
                exe_path: Some(c.exe_path.clone()),
                launch_args: Some(build_args(&c.args_template, &path)),
                process_matchers: matchers.clone(),
                size_bytes: std::fs::metadata(&rom).ok().map(|m| m.len() as i64),
                ..Default::default()
            }
        })
        .collect()
}

pub struct EmulatorSource {
    configs: Vec<EmulatorConfig>,
}

impl EmulatorSource {
    pub fn new(configs: Vec<EmulatorConfig>) -> Self {
        Self { configs }
    }
}

impl Source for EmulatorSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Emulator
    }

    fn status(&self) -> SourceStatus {
        if self.configs.is_empty() {
            SourceStatus::NotInstalled
        } else {
            SourceStatus::Connected
        }
    }

    fn scan(&self) -> AppResult<ScanResult> {
        let mut warnings = Vec::new();
        let mut games = Vec::new();
        for c in &self.configs {
            if !Path::new(&c.roms_dir).is_dir() {
                warnings.push(format!("{}: ROM folder not reachable", c.name));
                continue;
            }
            games.extend(roms(c));
        }
        Ok(ScanResult {
            kind: SourceKind::Emulator,
            status: SourceStatus::Connected,
            games,
            complete_ownership: true,
            warnings,
        })
    }
}

// ---------- RetroAchievements hashing (cartridge consoles) ----------

/// Console ids of RetroAchievements whose hash we can compute from the file.
pub fn hashable_console(console_id: u32) -> bool {
    matches!(console_id, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 11 | 15 | 25)
}

/// RetroAchievements hash of a cartridge ROM (rcheevos rules for the
/// consoles listed in `hashable_console`): plain MD5, minus the copier
/// header for NES and SNES, N64 normalized to big-endian (.z64).
pub fn ra_hash(console_id: u32, data: &[u8]) -> Option<String> {
    if !hashable_console(console_id) {
        return None;
    }
    let body: std::borrow::Cow<'_, [u8]> = match console_id {
        // NES: skip the 16-byte iNES header.
        7 if data.starts_with(b"NES\x1a") => data[16.min(data.len())..].into(),
        // SNES: skip a 512-byte copier header.
        3 if data.len() % 1024 == 512 => data[512..].into(),
        // N64: byte-swapped (.v64) or little-endian (.n64) → big-endian.
        2 => match data.first() {
            Some(0x37) => data
                .chunks(2)
                .flat_map(|c| c.iter().rev().copied())
                .collect::<Vec<u8>>()
                .into(),
            Some(0x40) => data
                .chunks(4)
                .flat_map(|c| c.iter().rev().copied())
                .collect::<Vec<u8>>()
                .into(),
            _ => data.into(),
        },
        _ => data.into(),
    };
    let mut h = Md5::new();
    h.update(&body);
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_rom_titles() {
        assert_eq!(
            clean_rom_title("Super Mario World (USA) [!]"),
            "Super Mario World"
        );
        assert_eq!(
            clean_rom_title("Final_Fantasy_VI (Europe) (En,Fr)"),
            "Final Fantasy VI"
        );
        assert_eq!(clean_rom_title("(Beta)"), "(Beta)");
    }

    #[test]
    fn builds_arguments() {
        assert_eq!(
            build_args("{rom}", r"D:\ROMs\a b.sfc"),
            r#""D:\ROMs\a b.sfc""#
        );
        assert_eq!(
            build_args(r#"-batch -fullscreen "{rom}""#, r"D:\x.iso"),
            r#"-batch -fullscreen "D:\x.iso""#
        );
        assert_eq!(build_args("-f", r"D:\x.gba"), r#"-f "D:\x.gba""#);
    }

    #[test]
    fn hashes_like_rcheevos() {
        // Plain MD5 for GBA.
        assert_eq!(
            ra_hash(5, b"abc").as_deref(),
            Some("900150983cd24fb0d6963f7d28e17f72")
        );
        // NES header ignored.
        let mut nes = b"NES\x1a".to_vec();
        nes.extend([0u8; 12]);
        nes.extend(b"abc");
        assert_eq!(ra_hash(7, &nes), ra_hash(5, b"abc"));
        // SNES copier header ignored.
        let mut snes = vec![0u8; 512];
        snes.extend(vec![7u8; 1024]);
        assert_eq!(ra_hash(3, &snes), ra_hash(3, &vec![7u8; 1024]));
        // N64 .v64 (16-bit swapped) and .n64 (little-endian) give the .z64 hash.
        let z64 = [0x80u8, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        let v64 = [0x37u8, 0x80, 0x40, 0x12, 2, 1, 4, 3];
        let n64 = [0x40u8, 0x12, 0x37, 0x80, 4, 3, 2, 1];
        assert_eq!(ra_hash(2, &v64), ra_hash(2, &z64));
        assert_eq!(ra_hash(2, &n64), ra_hash(2, &z64));
        // Disc consoles: no automatic hash.
        assert_eq!(ra_hash(12, b"abc"), None);
    }

    #[test]
    fn scans_roms_with_matching_extensions() {
        let dir = std::env::temp_dir().join(format!("ludotheque-roms-{}", crate::db::new_id()));
        std::fs::create_dir_all(dir.join("RPG")).unwrap();
        std::fs::write(dir.join("Super Metroid (USA).sfc"), b"x").unwrap();
        std::fs::write(dir.join("RPG").join("Chrono_Trigger (USA).SMC"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let cfg = EmulatorConfig {
            id: "snes9x".into(),
            name: "Snes9x".into(),
            exe_path: r"C:\Emu\snes9x-x64.exe".into(),
            args_template: "\"{rom}\"".into(),
            roms_dir: dir.display().to_string(),
            extensions: vec!["sfc".into(), "smc".into()],
            console_id: Some(3),
        };
        let games = roms(&cfg);
        let titles: Vec<_> = games.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Chrono Trigger", "Super Metroid"]);
        assert!(games[0]
            .launch_args
            .as_deref()
            .unwrap()
            .contains("Chrono_Trigger"));
        assert_eq!(games[0].external_id, games[0].external_id.to_lowercase());
        let _ = std::fs::remove_dir_all(dir);
    }
}
