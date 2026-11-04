//! Game launching. J1: platform URIs (`steam://rungameid/<appid>`).
//! Executables and launch profiles come with J3.

use crate::db::repo::game_sources::GameSource;
use crate::error::{AppError, AppResult};

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

/// Only platform schemes we generate ourselves are opened.
pub fn is_allowed_uri(uri: &str) -> bool {
    const SCHEMES: &[&str] = &["steam://"];
    SCHEMES.iter().any(|s| uri.starts_with(s))
}

pub fn launch(source: &GameSource) -> AppResult<()> {
    let uri = source
        .launch_uri
        .as_deref()
        .ok_or_else(|| AppError::Invalid("this source has no launch target yet".into()))?;
    if !is_allowed_uri(uri) {
        return Err(AppError::Invalid(format!("refusing to open '{uri}'")));
    }
    tracing::info!(%uri, "launching game");
    tauri_plugin_opener::open_url(uri, None::<&str>)
        .map_err(|e| AppError::Invalid(format!("cannot open {uri}: {e}")))
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

    #[test]
    fn only_known_schemes_are_allowed() {
        assert!(is_allowed_uri("steam://rungameid/10"));
        assert!(!is_allowed_uri("file:///C:/Windows/system32/cmd.exe"));
        assert!(!is_allowed_uri("https://example.com"));
    }
}
