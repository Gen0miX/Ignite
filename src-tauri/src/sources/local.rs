//! Games added by hand: an executable (`.exe`), a Windows shortcut (`.lnk`,
//! resolved to its target) or an internet shortcut (`.url`, launcher URI).

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::new_id;
use crate::db::repo::game_sources::{self, SourceData};
use crate::db::repo::games;
use crate::db::repo::sessions::{self, PlaySession, SessionOrigin};
use crate::error::{AppError, AppResult};
use crate::launcher::is_allowed_uri;
use crate::models::SourceKind;
use crate::tracker::matcher::ProcessMatcher;

/// What a picked file points to, to prefill the "add a game" form.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExeInfo {
    /// The file the user picked.
    pub picked: String,
    /// Executable to start (`.exe`, or the target of a `.lnk`).
    pub exe_path: Option<String>,
    /// Launcher URI (`.url` shortcut).
    pub uri: Option<String>,
    pub args: Option<String>,
    pub suggested_title: String,
    /// Crash handler, uninstaller, setup...: probably not the game itself.
    pub probably_not_a_game: bool,
}

/// SPEC § 4 "Faux jeux": executables that are not the game.
pub fn is_fake_exe(file_name: &str) -> bool {
    let n = file_name.to_lowercase();
    let n = n.trim_end_matches(".exe");
    [
        "crash",
        "unins",
        "setup",
        "redist",
        "vcredist",
        "dxsetup",
        "installer",
        "updater",
    ]
    .iter()
    .any(|p| n.contains(p))
        || n.starts_with("easyanticheat")
        || n.starts_with("beservice")
        || n.starts_with("battleye")
}

/// Executable names that say nothing about the game: use the folder instead.
fn is_generic_name(stem: &str) -> bool {
    let s = stem.to_lowercase();
    [
        "game", "launcher", "start", "play", "main", "app", "client", "win64", "win32", "x64",
    ]
    .contains(&s.as_str())
        || s.ends_with("-win64-shipping")
        || s.ends_with("-win32-shipping")
}

/// Folder name, skipping technical sub-folders (`bin`, `Binaries\Win64`...).
fn folder_title(exe: &Path) -> Option<String> {
    const TECH: &[&str] = &[
        "bin", "bin64", "binaries", "win64", "win32", "x64", "x86", "game", "release",
    ];
    exe.ancestors()
        .skip(1)
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .find(|n| !TECH.contains(&n.to_lowercase().as_str()))
}

/// Title guess: exe metadata (ProductName / FileDescription), then folder,
/// then file name.
pub fn suggest_title(exe: &Path, metadata: Option<(Option<String>, Option<String>)>) -> String {
    let stem = exe
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let (product, description) = metadata.unwrap_or_default();
    let clean = |s: Option<String>| {
        s.map(|v| v.trim().trim_matches('\0').to_string())
            .filter(|v| !v.is_empty() && !is_generic_name(v))
    };
    clean(product)
        .or(clean(description))
        .or_else(|| {
            (is_generic_name(&stem))
                .then(|| folder_title(exe))
                .flatten()
        })
        .unwrap_or(stem)
}

/// `ProductName` and `FileDescription` from the exe version resource.
#[cfg(windows)]
pub fn version_strings(exe: &Path) -> Option<(Option<String>, Option<String>)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let path: Vec<u16> = exe.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: plain Win32 calls with buffers we own and sizes the API gave us.
    unsafe {
        let size = GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len: u32 = 0;
        let mut lang = "040904b0".to_string();
        if VerQueryValueW(
            data.as_ptr().cast(),
            wide(r"\VarFileInfo\Translation").as_ptr(),
            &mut ptr,
            &mut len,
        ) != 0
            && len >= 4
        {
            let pair = std::slice::from_raw_parts(ptr as *const u16, 2);
            lang = format!("{:04x}{:04x}", pair[0], pair[1]);
        }
        let read = |name: &str| -> Option<String> {
            let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut l: u32 = 0;
            let key = wide(&format!(r"\StringFileInfo\{lang}\{name}"));
            if VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut p, &mut l) == 0 || l == 0 {
                return None;
            }
            let chars = std::slice::from_raw_parts(p as *const u16, l as usize);
            Some(
                String::from_utf16_lossy(chars)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        };
        Some((read("ProductName"), read("FileDescription")))
    }
}

#[cfg(not(windows))]
pub fn version_strings(_exe: &Path) -> Option<(Option<String>, Option<String>)> {
    None
}

/// `URL=` line of a `.url` internet shortcut.
pub fn parse_url_file(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("URL=").or_else(|| l.strip_prefix("url=")))
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
}

/// Resolves a `.lnk` shortcut to `(target, arguments)`. The `lnk` crate can
/// panic on unusual files, hence `catch_unwind`.
fn resolve_lnk(path: &Path) -> AppResult<(Option<String>, Option<String>)> {
    let path = path.to_path_buf();
    std::panic::catch_unwind(move || {
        let link = lnk::ShellLink::open(&path, lnk::encoding::WINDOWS_1252)
            .map_err(|e| AppError::Invalid(format!("unreadable shortcut: {e}")))?;
        let args = link.string_data().command_line_arguments().clone();
        Ok((link.link_target(), args.filter(|a| !a.trim().is_empty())))
    })
    .unwrap_or_else(|_| Err(AppError::Invalid("unreadable shortcut".into())))
}

pub fn inspect(picked: &Path) -> AppResult<ExeInfo> {
    if !picked.is_file() {
        return Err(AppError::NotFound(format!("file {}", picked.display())));
    }
    let ext = picked
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let stem_title = picked
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let (exe, uri, args) = match ext.as_str() {
        "exe" => (Some(picked.to_path_buf()), None, None),
        "lnk" => {
            let (target, args) = resolve_lnk(picked)?;
            (target.map(PathBuf::from), None, args)
        }
        "url" => {
            let text = String::from_utf8_lossy(&std::fs::read(picked)?).to_string();
            let url = parse_url_file(&text)
                .ok_or_else(|| AppError::Invalid("no URL in shortcut".into()))?;
            match url.strip_prefix("file:///") {
                Some(file) => (Some(PathBuf::from(file.replace('/', "\\"))), None, None),
                None => (None, Some(url), None),
            }
        }
        _ => return Err(AppError::Invalid("pick an .exe, .lnk or .url file".into())),
    };
    let suggested_title = match &exe {
        // A shortcut's own name is usually the game's name.
        Some(_) if ext != "exe" => stem_title.clone(),
        Some(e) => suggest_title(e, version_strings(e)),
        None => stem_title.clone(),
    };
    let probably_not_a_game = exe
        .as_ref()
        .and_then(|e| e.file_name())
        .is_some_and(|n| is_fake_exe(&n.to_string_lossy()));
    Ok(ExeInfo {
        picked: picked.display().to_string(),
        exe_path: exe.map(|e| e.display().to_string()),
        uri,
        args,
        suggested_title,
        probably_not_a_game,
    })
}

/// Which process the tracker follows for this game.
#[derive(Debug, Clone, PartialEq, Deserialize, specta::Type)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum TrackMode {
    /// The executable and, for a dedicated folder, anything inside it.
    Auto,
    /// Only the chosen executable.
    File,
    /// Another process by name (launcher that starts the real game).
    Process {
        #[serde(rename = "processName")]
        process_name: String,
    },
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddExeInput {
    pub title: String,
    pub exe_path: Option<String>,
    pub uri: Option<String>,
    pub args: Option<String>,
    pub track: TrackMode,
    /// "Temps déjà joué", recorded as an `estimate` session.
    pub estimate_minutes: Option<u32>,
}

/// A folder is "dedicated" to a game when it is not a drive root or a shared
/// system folder; only then can any exe inside it count as the game.
pub fn is_dedicated_folder(dir: &Path) -> bool {
    const SHARED: &[&str] = &[
        "program files",
        "program files (x86)",
        "windows",
        "desktop",
        "bureau",
        "downloads",
        "téléchargements",
        "documents",
        "games",
        "jeux",
    ];
    let depth = dir.components().count();
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    depth >= 2 && !name.is_empty() && !SHARED.contains(&name.as_str())
}

pub fn matchers_for(exe: Option<&Path>, track: &TrackMode) -> Vec<ProcessMatcher> {
    match track {
        TrackMode::Process { process_name } => {
            let name = process_name.trim();
            if name.is_empty() {
                Vec::new()
            } else {
                let name = if name.to_lowercase().ends_with(".exe") {
                    name.to_string()
                } else {
                    format!("{name}.exe")
                };
                vec![ProcessMatcher::Exe(name)]
            }
        }
        TrackMode::File => exe
            .map(|e| vec![ProcessMatcher::ExePath(e.display().to_string())])
            .unwrap_or_default(),
        TrackMode::Auto => {
            let Some(e) = exe else { return Vec::new() };
            let mut list = vec![ProcessMatcher::ExePath(e.display().to_string())];
            if let Some(dir) = e.parent().filter(|d| is_dedicated_folder(d)) {
                list.push(ProcessMatcher::PathPrefix(dir.display().to_string()));
            }
            list
        }
    }
}

/// Creates the game, its `local` source, the default launch profile and the
/// optional estimate. Returns the game id.
pub fn create(conn: &mut Connection, input: &AddExeInput) -> AppResult<String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::Invalid("title is required".into()));
    }
    let exe = input
        .exe_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    let uri = input
        .uri
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    if let Some(e) = &exe {
        let ext = e
            .extension()
            .map(|x| x.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !e.is_file() || !["exe", "bat", "cmd"].contains(&ext.as_str()) {
            return Err(AppError::Invalid("the executable does not exist".into()));
        }
    }
    if let Some(u) = &uri {
        if !is_allowed_uri(u) {
            return Err(AppError::Invalid("unsupported launcher link".into()));
        }
    }
    let external_id = match (&exe, &uri) {
        (Some(e), _) => e.display().to_string().to_lowercase(),
        (None, Some(u)) => u.clone(),
        (None, None) => {
            return Err(AppError::Invalid(
                "an executable or a link is required".into(),
            ))
        }
    };
    if game_sources::find_by_external(conn, SourceKind::Local, &external_id)?.is_some() {
        return Err(AppError::Invalid(
            "this executable is already in the library".into(),
        ));
    }
    let matchers = matchers_for(exe.as_deref(), &input.track);
    let tx = conn.transaction()?;
    let game = games::insert(&tx, title)?;
    let source = game_sources::upsert(
        &tx,
        &game.id,
        SourceKind::Local,
        &SourceData {
            external_id: Some(external_id),
            installed: true,
            install_path: exe
                .as_ref()
                .and_then(|e| e.parent())
                .map(|d| d.display().to_string()),
            exe_path: exe.as_ref().map(|e| e.display().to_string()),
            launch_uri: uri,
            process_matchers: (!matchers.is_empty())
                .then(|| serde_json::to_string(&matchers))
                .transpose()?,
            ..Default::default()
        },
    )?;
    let args = input
        .args
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty());
    tx.execute(
        "INSERT INTO launch_profiles (id, game_source_id, name, is_default, mode, args) \
         VALUES (?1, ?2, 'Par défaut', 1, 'auto', ?3)",
        params![new_id(), source.id, args],
    )?;
    if let Some(minutes) = input.estimate_minutes.filter(|m| *m > 0) {
        sessions::save(
            &tx,
            &PlaySession {
                id: new_id(),
                game_id: game.id.clone(),
                game_source_id: Some(source.id.clone()),
                started_at: None,
                ended_at: None,
                duration_seconds: minutes.saturating_mul(60),
                idle_seconds: 0,
                origin: SessionOrigin::Estimate,
                note: None,
                rating: None,
            },
        )?;
    }
    tx.commit()?;
    Ok(game.id)
}

/// Arguments of the default launch profile of a source, if any.
pub fn default_args(conn: &Connection, source_id: &str) -> AppResult<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT args FROM launch_profiles WHERE game_source_id = ?1 AND is_default = 1",
            [source_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::library::query;

    #[test]
    fn flags_fake_executables() {
        for n in [
            "UnityCrashHandler64.exe",
            "unins000.exe",
            "Setup.exe",
            "vcredist_x64.exe",
            "EasyAntiCheat_Setup.exe",
            "BEService_x64.exe",
        ] {
            assert!(is_fake_exe(n), "{n}");
        }
        for n in ["Celeste.exe", "Hades.exe", "Minecraft.exe"] {
            assert!(!is_fake_exe(n), "{n}");
        }
    }

    #[test]
    fn suggests_titles_from_metadata_then_folder() {
        let exe = Path::new(r"D:\Games\Celeste\Celeste.exe");
        assert_eq!(
            suggest_title(exe, Some((Some("Celeste".into()), None))),
            "Celeste"
        );
        assert_eq!(
            suggest_title(exe, Some((Some("  ".into()), Some("Celeste Game".into())))),
            "Celeste Game"
        );
        assert_eq!(suggest_title(exe, None), "Celeste");
        let generic = Path::new(r"D:\Games\Hollow Knight\bin\Win64\Game.exe");
        assert_eq!(
            suggest_title(generic, Some((Some("Launcher".into()), None))),
            "Hollow Knight"
        );
        let shipping = Path::new(r"D:\Games\Satisfactory\FactoryGame-Win64-Shipping.exe");
        assert_eq!(suggest_title(shipping, None), "Satisfactory");
    }

    #[test]
    fn parses_url_shortcuts() {
        let text = "[InternetShortcut]\r\nURL=com.epicgames.launcher://apps/Fortnite?action=launch\r\nIconIndex=0\r\n";
        assert_eq!(
            parse_url_file(text).as_deref(),
            Some("com.epicgames.launcher://apps/Fortnite?action=launch")
        );
        assert_eq!(parse_url_file("[InternetShortcut]\n"), None);
    }

    #[test]
    fn matchers_follow_the_tracking_mode() {
        let exe = Path::new(r"D:\Games\Celeste\Celeste.exe");
        assert_eq!(
            matchers_for(Some(exe), &TrackMode::Auto),
            vec![
                ProcessMatcher::ExePath(r"D:\Games\Celeste\Celeste.exe".into()),
                ProcessMatcher::PathPrefix(r"D:\Games\Celeste".into())
            ]
        );
        // An exe dropped in a shared folder: never "anything in the folder".
        let loose = Path::new(r"C:\Program Files\tool.exe");
        assert_eq!(matchers_for(Some(loose), &TrackMode::Auto).len(), 1);
        assert_eq!(matchers_for(Some(exe), &TrackMode::File).len(), 1);
        assert_eq!(
            matchers_for(
                Some(exe),
                &TrackMode::Process {
                    process_name: "javaw".into()
                }
            ),
            vec![ProcessMatcher::Exe("javaw.exe".into())]
        );
        assert!(!is_dedicated_folder(Path::new(r"D:\")));
        assert!(!is_dedicated_folder(Path::new(r"C:\Users\me\Desktop")));
    }

    #[test]
    fn creates_a_local_game_with_profile_and_estimate() {
        let dir = std::env::temp_dir()
            .join(format!("ludotheque-exe-{}", new_id()))
            .join("Celeste");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Celeste.exe");
        std::fs::write(&exe, b"MZ").unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut conn = db.conn();
        let input = AddExeInput {
            title: "Celeste".into(),
            exe_path: Some(exe.display().to_string()),
            uri: None,
            args: Some("-windowed".into()),
            track: TrackMode::Auto,
            estimate_minutes: Some(600),
        };
        let id = create(&mut conn, &input).unwrap();
        let summary = query::summaries(&conn)
            .unwrap()
            .into_iter()
            .find(|g| g.id == id)
            .unwrap();
        assert_eq!(summary.sources, vec![SourceKind::Local]);
        assert_eq!(summary.playtime_minutes, 600);
        let source = game_sources::list_for_game(&conn, &id).unwrap().remove(0);
        assert_eq!(
            default_args(&conn, &source.id).unwrap().as_deref(),
            Some("-windowed")
        );
        assert!(source.process_matchers.unwrap().contains("path_prefix"));

        // Same exe twice is refused; bad input too.
        assert!(create(&mut conn, &input).is_err());
        let mut bad = input.clone();
        bad.exe_path = Some(dir.join("missing.exe").display().to_string());
        assert!(create(&mut conn, &bad).is_err());
        let mut uri = input.clone();
        uri.exe_path = None;
        uri.uri = Some("https://example.com".into());
        assert!(create(&mut conn, &uri).is_err());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn inspects_exe_and_url_files() {
        let dir = std::env::temp_dir().join(format!("ludotheque-inspect-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Celeste.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let info = inspect(&exe).unwrap();
        assert_eq!(
            info.exe_path.as_deref(),
            Some(exe.display().to_string().as_str())
        );
        assert_eq!(info.suggested_title, "Celeste");
        let url = dir.join("Fortnite.url");
        std::fs::write(
            &url,
            "[InternetShortcut]\nURL=com.epicgames.launcher://apps/Fortnite\n",
        )
        .unwrap();
        let info = inspect(&url).unwrap();
        assert_eq!(
            info.uri.as_deref(),
            Some("com.epicgames.launcher://apps/Fortnite")
        );
        assert_eq!(info.suggested_title, "Fortnite");
        assert!(inspect(&dir.join("notes.txt")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Reads version metadata from a real Windows executable.
    #[cfg(windows)]
    #[test]
    fn reads_version_info_of_notepad() {
        let notepad = Path::new(r"C:\Windows\System32\notepad.exe");
        if notepad.is_file() {
            let (product, description) = version_strings(notepad).unwrap();
            assert!(product.is_some() || description.is_some());
        }
    }
}
