//! Tauri commands. Every command is annotated with `#[specta::specta]` so its
//! TypeScript signature is generated into `src/api/bindings.ts`.

pub mod achievements;
pub mod app;
pub mod artwork;
pub mod collections;
pub mod emulators;
pub mod home;
pub mod import;
pub mod library;
pub mod media;
pub mod play;
pub mod recap;
pub mod secrets;
pub mod sources;
pub mod tracking;

use serde::Serialize;

use crate::error::AppError;

/// Error shape returned to the frontend by every command.
#[derive(Debug, Serialize, specta::Type)]
pub struct CommandError {
    /// Stable machine-readable category, e.g. `not_found`, `invalid`.
    pub kind: String,
    /// Human-readable message (English, for logs; the UI shows its own text).
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(e: AppError) -> Self {
        let kind = match &e {
            AppError::Db(_) | AppError::Migration(_) => "database",
            AppError::Json(_) => "serialization",
            AppError::Io(_) => "io",
            AppError::Network { .. } => "network",
            AppError::Secret(_) => "secret_store",
            AppError::NotFound(_) => "not_found",
            AppError::Invalid(_) => "invalid",
        };
        tracing::warn!(kind, error = %e, "command failed");
        CommandError {
            kind: kind.to_string(),
            message: e.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

/// Builder holding every command; shared by `run()` and the bindings export.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        app::app_info,
        secrets::secrets_status,
        secrets::secrets_set,
        secrets::secrets_clear,
        library::library_list,
        library::library_counts,
        library::game_get,
        library::game_update,
        library::game_tag_add,
        library::game_tag_remove,
        library::launch_game,
        library::duplicates_list,
        library::game_merge,
        library::game_split,
        library::exe_inspect,
        library::add_executable,
        library::facets,
        library::folders_scan,
        library::add_scanned_games,
        artwork::artwork_search,
        artwork::artwork_set,
        collections::collections_list,
        collections::collection_create,
        collections::collection_update,
        collections::collection_delete,
        collections::collection_set_member,
        collections::rules_preview,
        collections::search_check,
        collections::game_collections,
        collections::stats_overview,
        tracking::tracking_settings_get,
        tracking::tracking_settings_set,
        emulators::emulators_get,
        emulators::emulators_set,
        emulators::ra_consoles,
        emulators::ra_username_get,
        emulators::ra_username_set,
        emulators::game_set_ra_id,
        home::home_overview,
        home::home_tonight,
        home::home_news,
        home::open_link,
        media::game_store_media,
        media::game_news,
        home::couch_shelf,
        home::couch_wheel,
        app::app_settings_get,
        app::app_settings_set,
        library::games_bulk_update,
        library::games_delete,
        library::source_open_folder,
        library::launch_game_source,
        recap::session_recap,
        recap::session_recap_save,
        recap::session_discard,
        recap::window_reveal,
        recap::notify,
        collections::collection_add_games,
        sources::library_sync,
        sources::sources_status,
        sources::source_set_enabled,
        import::import_scan,
        import::import_commit,
        sources::steam_settings_get,
        sources::steam_settings_set,
        play::tracker_active,
        play::sessions_list,
        play::session_save,
        play::session_delete,
        play::goals_list,
        play::goal_add,
        play::goal_toggle,
        play::goal_rename,
        play::goal_delete,
        achievements::achievements_get,
    ])
}

/// Path of the generated TypeScript bindings.
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/api/bindings.ts");

pub fn export_bindings() {
    specta_builder()
        .export(
            // Regenerate with `pnpm bindings` (also refreshed by every debug launch).
            specta_typescript::Typescript::default().header("// @ts-nocheck"),
            BINDINGS_PATH,
        )
        .expect("failed to export TypeScript bindings");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates `src/api/bindings.ts` (`pnpm bindings` runs this test).
    #[test]
    fn export_typescript_bindings() {
        export_bindings();
        let content = std::fs::read_to_string(BINDINGS_PATH).unwrap();
        assert!(content.contains("secretsSet"));
        // The read side of secrets must never be exposed to the frontend.
        assert!(!content.contains("secretsGet"));
    }
}
