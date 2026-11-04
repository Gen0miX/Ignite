//! Which running process belongs to which game (ARCHITECTURE.md § 4).
//!
//! Priority when several games could match a process:
//! 1. exact executable path, 2. install-folder prefix (longest wins),
//! 3. executable name.

use serde::{Deserialize, Serialize};

/// Stored as JSON in `game_sources.process_matchers`, e.g.
/// `[{"exe":"javaw.exe"}, {"path_prefix":"D:\\Games\\X"}]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ProcessMatcher {
    ExePath(String),
    PathPrefix(String),
    Exe(String),
}

/// A game source the tracker watches.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchEntry {
    pub game_id: String,
    pub source_id: String,
    pub matchers: Vec<ProcessMatcher>,
}

/// A running process as seen by the system probe.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: Option<String>,
}

/// Matchers of a source: explicit JSON if set, else derived from the install
/// folder, else from the executable.
pub fn matchers_for(
    explicit_json: Option<&str>,
    install_path: Option<&str>,
    exe_path: Option<&str>,
) -> Vec<ProcessMatcher> {
    if let Some(json) = explicit_json {
        match serde_json::from_str::<Vec<ProcessMatcher>>(json) {
            Ok(list) if !list.is_empty() => return list,
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "invalid process_matchers JSON, using defaults"),
        }
    }
    let mut list = Vec::new();
    if let Some(p) = exe_path.filter(|p| !p.trim().is_empty()) {
        list.push(ProcessMatcher::ExePath(p.to_string()));
    }
    if let Some(p) = install_path.filter(|p| !p.trim().is_empty()) {
        list.push(ProcessMatcher::PathPrefix(p.to_string()));
    }
    list
}

/// Windows paths: case-insensitive, `/` and `\` equivalent, no trailing separator.
fn norm(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Score of `matcher` for `process`; `None` when it does not match.
/// Higher is better: exact path > prefix (longer is better) > name.
fn score(matcher: &ProcessMatcher, process: &ProcessInfo) -> Option<usize> {
    match matcher {
        ProcessMatcher::ExePath(p) => {
            let exe = process.exe_path.as_deref()?;
            (norm(exe) == norm(p)).then_some(3_000_000)
        }
        ProcessMatcher::PathPrefix(prefix) => {
            let exe = norm(process.exe_path.as_deref()?);
            let prefix = norm(prefix);
            // Guard against drive roots or empty prefixes matching everything.
            if prefix.len() < 4 {
                return None;
            }
            exe.strip_prefix(&prefix)
                .filter(|rest| rest.starts_with('\\'))
                .map(|_| 1_000_000 + prefix.len())
        }
        ProcessMatcher::Exe(name) => process.name.eq_ignore_ascii_case(name.trim()).then_some(1),
    }
}

/// `(game_id, source_id)` of every game with at least one running process.
/// Each process is attributed to its single best-matching source.
pub fn running_games(watch: &[WatchEntry], processes: &[ProcessInfo]) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    for process in processes {
        let best = watch
            .iter()
            .filter_map(|entry| {
                entry
                    .matchers
                    .iter()
                    .filter_map(|m| score(m, process))
                    .max()
                    .map(|s| (s, entry))
            })
            .max_by_key(|(s, _)| *s);
        if let Some((_, entry)) = best {
            let key = (entry.game_id.clone(), entry.source_id.clone());
            if !found.contains(&key) {
                found.push(key);
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(name: &str, exe: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid: 1,
            name: name.into(),
            exe_path: exe.map(Into::into),
        }
    }

    fn entry(game: &str, matchers: Vec<ProcessMatcher>) -> WatchEntry {
        WatchEntry {
            game_id: game.into(),
            source_id: format!("src-{game}"),
            matchers,
        }
    }

    #[test]
    fn parses_the_documented_json_format() {
        let list = matchers_for(
            Some(r#"[{"exe":"javaw.exe"},{"path_prefix":"D:\\Games\\X"}]"#),
            None,
            None,
        );
        assert_eq!(
            list,
            vec![
                ProcessMatcher::Exe("javaw.exe".into()),
                ProcessMatcher::PathPrefix(r"D:\Games\X".into())
            ]
        );
    }

    #[test]
    fn derives_defaults_and_survives_bad_json() {
        let list = matchers_for(
            Some("not json"),
            Some(r"C:\Steam\steamapps\common\Hades"),
            None,
        );
        assert_eq!(
            list,
            vec![ProcessMatcher::PathPrefix(
                r"C:\Steam\steamapps\common\Hades".into()
            )]
        );
        assert!(matchers_for(None, None, None).is_empty());
    }

    #[test]
    fn prefix_matches_whole_folders_case_insensitively() {
        let watch = [entry(
            "hades",
            vec![ProcessMatcher::PathPrefix(
                r"C:\Steam\steamapps\common\Hades".into(),
            )],
        )];
        let inside = proc(
            "Hades.exe",
            Some(r"c:/steam/steamapps/common/hades/x64/Hades.exe"),
        );
        let sibling = proc(
            "Hades2.exe",
            Some(r"C:\Steam\steamapps\common\Hades II\Hades2.exe"),
        );
        assert_eq!(
            running_games(&watch, &[inside]),
            vec![("hades".into(), "src-hades".into())]
        );
        assert!(running_games(&watch, &[sibling]).is_empty());
    }

    #[test]
    fn exact_path_beats_prefix_and_longer_prefix_wins() {
        let watch = [
            entry(
                "outer",
                vec![ProcessMatcher::PathPrefix(r"D:\Games".into())],
            ),
            entry(
                "inner",
                vec![ProcessMatcher::PathPrefix(r"D:\Games\Celeste".into())],
            ),
            entry(
                "exact",
                vec![ProcessMatcher::ExePath(
                    r"D:\Games\Celeste\Celeste.exe".into(),
                )],
            ),
        ];
        let p = proc("Celeste.exe", Some(r"D:\Games\Celeste\Celeste.exe"));
        assert_eq!(
            running_games(&watch, &[p]),
            vec![("exact".into(), "src-exact".into())]
        );
        let q = proc("Tool.exe", Some(r"D:\Games\Celeste\tools\Tool.exe"));
        assert_eq!(
            running_games(&watch, &[q]),
            vec![("inner".into(), "src-inner".into())]
        );
    }

    #[test]
    fn name_matching_works_without_a_path() {
        let watch = [entry("mc", vec![ProcessMatcher::Exe("javaw.exe".into())])];
        assert_eq!(running_games(&watch, &[proc("JAVAW.EXE", None)]).len(), 1);
        // A prefix matcher needs the path: elevated processes without a path are ignored.
        let watch = [entry(
            "x",
            vec![ProcessMatcher::PathPrefix(r"C:\Games\X".into())],
        )];
        assert!(running_games(&watch, &[proc("X.exe", None)]).is_empty());
    }

    #[test]
    fn several_processes_of_one_game_count_once_and_roots_never_match() {
        let watch = [
            entry("g", vec![ProcessMatcher::PathPrefix(r"C:\Games\G".into())]),
            entry("root", vec![ProcessMatcher::PathPrefix(r"C:\".into())]),
        ];
        let procs = [
            proc("G.exe", Some(r"C:\Games\G\G.exe")),
            proc(
                "UnityCrashHandler64.exe",
                Some(r"C:\Games\G\UnityCrashHandler64.exe"),
            ),
            proc("explorer.exe", Some(r"C:\Windows\explorer.exe")),
        ];
        assert_eq!(
            running_games(&watch, &procs),
            vec![("g".into(), "src-g".into())]
        );
    }
}
