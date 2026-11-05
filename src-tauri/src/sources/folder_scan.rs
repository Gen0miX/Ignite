//! "Scanner ce PC": each sub-folder of a chosen directory (`D:\Games`) is a
//! candidate game; its main executable is guessed (SPEC § 4 "Faux jeux").

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::sources::local::{is_fake_exe, suggest_title, version_strings};

/// Folders that never contain the game's own executable.
const SKIPPED_DIRS: &[&str] = &[
    "_commonredist",
    "commonredist",
    "redist",
    "redistributables",
    "directx",
    "support",
    "__installer",
    "installer",
    "easyanticheat",
    "battleye",
    "vcredist",
    "dotnet",
    "prerequisites",
    "crashreporter",
    "tools",
];

/// Depth searched below each game folder.
const MAX_DEPTH: usize = 3;
/// Safety cap on directory entries visited per game folder.
const MAX_ENTRIES: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderCandidate {
    pub folder: String,
    pub exe_path: String,
    pub title: String,
    /// Other executables found (to pick another one in the UI).
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone)]
struct Exe {
    path: PathBuf,
    depth: usize,
    size: u64,
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<Exe>, visited: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        *visited += 1;
        if *visited > MAX_ENTRIES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            if depth < MAX_DEPTH && !SKIPPED_DIRS.contains(&name.as_str()) {
                collect(&path, depth + 1, out, visited);
            }
        } else if name.ends_with(".exe") && !is_fake_exe(&name) {
            out.push(Exe {
                path,
                depth,
                size: meta.len(),
            });
        }
    }
}

/// Main executable of a game folder: shallowest first, then the largest.
pub fn best_exe(folder: &Path) -> Option<(PathBuf, Vec<PathBuf>)> {
    let mut exes = Vec::new();
    let mut visited = 0;
    collect(folder, 0, &mut exes, &mut visited);
    exes.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then(b.size.cmp(&a.size))
            .then(a.path.cmp(&b.path))
    });
    let mut iter = exes.into_iter().map(|e| e.path);
    let best = iter.next()?;
    Some((best, iter.take(5).collect()))
}

fn norm(p: &Path) -> String {
    p.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Candidates under `roots`, skipping folders already in the library
/// (`known`: install folders and executables of existing sources).
pub fn scan(roots: &[PathBuf], known: &[String]) -> Vec<FolderCandidate> {
    let known: Vec<String> = known.iter().map(|k| norm(Path::new(k))).collect();
    let is_known = |p: &Path| {
        let n = norm(p);
        known.iter().any(|k| {
            k == &n || k.starts_with(&format!("{n}\\")) || n.starts_with(&format!("{k}\\"))
        })
    };
    let mut out = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        let mut folders: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        folders.sort();
        for folder in folders {
            if is_known(&folder) {
                continue;
            }
            let Some((exe, others)) = best_exe(&folder) else {
                continue;
            };
            let folder_name = folder
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let suggested = suggest_title(&exe, version_strings(&exe));
            // A generic or technical exe name gives a worse title than the folder.
            let title = if suggested.eq_ignore_ascii_case(
                exe.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_ref(),
            ) {
                folder_name.clone()
            } else {
                suggested
            };
            out.push(FolderCandidate {
                folder: folder.display().to_string(),
                exe_path: exe.display().to_string(),
                title,
                alternatives: others.iter().map(|p| p.display().to_string()).collect(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path, size: usize) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, vec![0u8; size]).unwrap();
    }

    #[test]
    fn picks_the_main_exe_and_skips_known_folders() {
        let root = std::env::temp_dir().join(format!("ludotheque-scan-{}", crate::db::new_id()));
        // Celeste: exe at the root, plus a crash handler and redist.
        touch(&root.join("Celeste").join("Celeste.exe"), 2000);
        touch(&root.join("Celeste").join("UnityCrashHandler64.exe"), 9000);
        touch(
            &root
                .join("Celeste")
                .join("_CommonRedist")
                .join("vc_redist.x64.exe"),
            9000,
        );
        // Hollow Knight: exe deeper, generic name; the bigger one wins.
        touch(
            &root.join("Hollow Knight").join("bin").join("Game.exe"),
            5000,
        );
        touch(
            &root.join("Hollow Knight").join("bin").join("Launcher.exe"),
            100,
        );
        // Only tools: no candidate.
        touch(&root.join("Drivers").join("setup.exe"), 10);
        // Already in the library.
        touch(&root.join("Hades").join("Hades.exe"), 10);

        let known = vec![root.join("Hades").display().to_string()];
        let found = scan(std::slice::from_ref(&root), &known);
        let titles: Vec<_> = found.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Celeste", "Hollow Knight"]);
        assert!(found[0].exe_path.ends_with("Celeste.exe"));
        assert!(found[1].exe_path.ends_with("Game.exe"));
        assert_eq!(found[1].alternatives.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_roots_are_ignored() {
        assert!(scan(&[PathBuf::from(r"Z:\nope")], &[]).is_empty());
    }
}
