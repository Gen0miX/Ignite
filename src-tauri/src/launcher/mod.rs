//! Game launching (ARCHITECTURE.md § 5): a platform URI (Steam, Epic...) or
//! an executable started detached, so closing Ludothèque never kills a game.

use std::path::{Path, PathBuf};

use crate::db::repo::game_sources::GameSource;
use crate::error::{AppError, AppResult};
use crate::models::SourceKind;

/// Source used by the Play button: the preferred one if it can be launched,
/// else an installed source, else any launchable source.
pub fn pick_source<'a>(
    sources: &'a [GameSource],
    preferred: Option<&str>,
) -> Option<&'a GameSource> {
    let launchable = |s: &&GameSource| s.launch_uri.is_some() || s.exe_path.is_some();
    preferred
        .and_then(|id| sources.iter().filter(launchable).find(|s| s.id == id))
        .or_else(|| sources.iter().filter(launchable).find(|s| s.installed))
        .or_else(|| sources.iter().find(launchable))
}

/// Launcher schemes we accept. Anything else (file:, http:, ...) is refused.
const SCHEMES: &[&str] = &[
    "steam://",
    "com.epicgames.launcher://",
    "goggalaxy://",
    "uplay://",
    "battlenet://",
    "origin2://",
    "link2ea://",
];

pub fn is_allowed_uri(uri: &str) -> bool {
    let lower = uri.trim().to_lowercase();
    SCHEMES.iter().any(|s| lower.starts_with(s))
}

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchTarget {
    Uri(String),
    Exe {
        path: PathBuf,
        args: Vec<String>,
        cwd: PathBuf,
    },
}

/// Splits a command line like Windows does for simple cases: spaces separate
/// arguments, double quotes group them.
pub fn split_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_token = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_token {
                    args.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(current);
    }
    args
}

fn exe_target(exe: &str, args: Vec<String>) -> LaunchTarget {
    let path = PathBuf::from(exe);
    let cwd = path.parent().map(Path::to_path_buf).unwrap_or_default();
    LaunchTarget::Exe { path, args, cwd }
}

/// What to start for a source. `galaxy_client`: GalaxyClient.exe if found;
/// `extra_args`: arguments of the launch profile (executables only).
pub fn resolve_target(
    source: &GameSource,
    galaxy_client: Option<&Path>,
    extra_args: Option<&str>,
) -> AppResult<LaunchTarget> {
    let args = || extra_args.map(split_args).unwrap_or_default();
    match source.kind {
        SourceKind::Gog => {
            let id = source.external_id.as_deref().unwrap_or_default();
            match (galaxy_client, &source.install_path) {
                // Through Galaxy: cloud saves, overlay and its own play tasks.
                (Some(client), Some(dir)) if !id.is_empty() => {
                    let cwd = client.parent().map(Path::to_path_buf).unwrap_or_default();
                    Ok(LaunchTarget::Exe {
                        path: client.to_path_buf(),
                        args: vec![
                            "/command=runGame".into(),
                            format!("/gameId={id}"),
                            format!("/path={dir}"),
                        ],
                        cwd,
                    })
                }
                _ => source
                    .exe_path
                    .as_deref()
                    .map(|exe| exe_target(exe, args()))
                    .ok_or_else(|| AppError::Invalid("GOG game is not installed".into())),
            }
        }
        SourceKind::Local | SourceKind::Emulator => source
            .exe_path
            .as_deref()
            .map(|exe| exe_target(exe, args()))
            .or_else(|| source.launch_uri.clone().map(LaunchTarget::Uri))
            .ok_or_else(|| AppError::Invalid("no executable for this game".into())),
        _ => source
            .launch_uri
            .clone()
            .map(LaunchTarget::Uri)
            .or_else(|| {
                source
                    .exe_path
                    .as_deref()
                    .map(|exe| exe_target(exe, args()))
            })
            .ok_or_else(|| AppError::Invalid("this source has no launch target".into())),
    }
}

pub fn execute(target: &LaunchTarget) -> AppResult<()> {
    match target {
        LaunchTarget::Uri(uri) => {
            if !is_allowed_uri(uri) {
                return Err(AppError::Invalid(format!("refusing to open '{uri}'")));
            }
            tracing::info!(%uri, "launching game");
            tauri_plugin_opener::open_url(uri, None::<&str>)
                .map_err(|e| AppError::Invalid(format!("cannot open {uri}: {e}")))
        }
        LaunchTarget::Exe { path, args, cwd } => {
            if !path.is_file() {
                return Err(AppError::NotFound(format!("executable {}", path.display())));
            }
            tracing::info!(exe = %path.display(), ?args, "launching executable");
            let mut cmd = std::process::Command::new(path);
            cmd.args(args);
            if cwd.is_dir() {
                cmd.current_dir(cwd);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const DETACHED_PROCESS: u32 = 0x0000_0008;
                const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
                cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
            }
            cmd.spawn().map(|_| ()).map_err(AppError::from)
        }
    }
}

/// Picks the target and starts it.
pub fn launch(source: &GameSource, extra_args: Option<&str>) -> AppResult<()> {
    let client = if source.kind == SourceKind::Gog {
        crate::sources::gog::locate_client()
    } else {
        None
    };
    execute(&resolve_target(source, client.as_deref(), extra_args)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SourceKind;

    fn src(id: &str, installed: bool, uri: Option<&str>) -> GameSource {
        GameSource {
            id: id.into(),
            game_id: "g".into(),
            kind: SourceKind::Steam,
            external_id: Some(id.into()),
            installed,
            install_path: None,
            exe_path: None,
            size_bytes: None,
            launch_uri: uri.map(Into::into),
            process_matchers: None,
            source_playtime_minutes: None,
            source_last_played: None,
            last_synced_at: None,
        }
    }

    #[test]
    fn picks_preferred_then_installed_then_any() {
        let list = vec![
            src("a", false, Some("steam://rungameid/1")),
            src("b", true, Some("steam://rungameid/2")),
            src("c", true, None),
        ];
        assert_eq!(pick_source(&list, Some("a")).unwrap().id, "a");
        assert_eq!(pick_source(&list, None).unwrap().id, "b");
        assert_eq!(pick_source(&list, Some("c")).unwrap().id, "b"); // c is not launchable
        assert_eq!(pick_source(&list[..1], None).unwrap().id, "a");
        assert!(pick_source(&list[2..], None).is_none());
    }

    fn with(kind: SourceKind, f: impl FnOnce(&mut GameSource)) -> GameSource {
        let mut s = src("1207664643", true, None);
        s.kind = kind;
        f(&mut s);
        s
    }

    #[test]
    fn resolves_targets_per_platform() {
        let steam = src("1", true, Some("steam://rungameid/1"));
        assert_eq!(
            resolve_target(&steam, None, None).unwrap(),
            LaunchTarget::Uri("steam://rungameid/1".into())
        );

        let gog = with(SourceKind::Gog, |s| {
            s.install_path = Some(r"C:\Games\W3".into());
            s.exe_path = Some(r"C:\Games\W3\REDprelauncher.exe".into());
        });
        let client = Path::new(r"C:\Program Files\GOG Galaxy\GalaxyClient.exe");
        match resolve_target(&gog, Some(client), None).unwrap() {
            LaunchTarget::Exe { path, args, .. } => {
                assert_eq!(path, client);
                assert_eq!(
                    args,
                    [
                        "/command=runGame",
                        "/gameId=1207664643",
                        r"/path=C:\Games\W3"
                    ]
                );
            }
            other => panic!("{other:?}"),
        }
        // Without Galaxy: the game's own executable, started from its folder.
        match resolve_target(&gog, None, Some("-skipintro")).unwrap() {
            LaunchTarget::Exe { path, args, cwd } => {
                assert_eq!(path, PathBuf::from(r"C:\Games\W3\REDprelauncher.exe"));
                assert_eq!(args, ["-skipintro"]);
                assert_eq!(cwd, PathBuf::from(r"C:\Games\W3"));
            }
            other => panic!("{other:?}"),
        }

        let local = with(SourceKind::Local, |s| {
            s.exe_path = Some(r"D:\Games\Celeste\Celeste.exe".into())
        });
        assert!(matches!(
            resolve_target(&local, None, None).unwrap(),
            LaunchTarget::Exe { .. }
        ));
        let nothing = with(SourceKind::Local, |_| {});
        assert!(resolve_target(&nothing, None, None).is_err());
    }

    #[test]
    fn splits_arguments_with_quotes() {
        assert_eq!(
            split_args(r#"-fullscreen -dir "C:\My Games" """#),
            ["-fullscreen", "-dir", r"C:\My Games", ""]
        );
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn only_known_schemes_are_allowed() {
        assert!(is_allowed_uri("steam://rungameid/10"));
        assert!(is_allowed_uri(
            "com.epicgames.launcher://apps/Dill?action=launch"
        ));
        assert!(!is_allowed_uri("file:///C:/Windows/system32/cmd.exe"));
        assert!(!is_allowed_uri("https://example.com"));
    }
}
