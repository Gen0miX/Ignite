use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use super::CommandResult;
use crate::db::repo::settings;
use crate::db::Db;
use crate::error::AppError;
use crate::tray;

/// Filesystem locations used by the app, stored in Tauri state at startup.
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
}

/// Basic app info, shown in the settings footer and useful to check the IPC bridge.
#[tauri::command]
#[specta::specta]
pub fn app_info(paths: tauri::State<'_, AppPaths>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: paths.data_dir.display().to_string(),
        db_path: paths.db_path.display().to_string(),
    }
}

/// Application behaviour (Réglages › Application).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Closing the window keeps Ignite in the notification area.
    pub close_to_tray: bool,
    /// Start with Windows, minimized in the notification area.
    pub autostart: bool,
}

#[tauri::command]
#[specta::specta]
pub fn app_settings_get(app: AppHandle) -> AppSettings {
    AppSettings {
        close_to_tray: tray::close_to_tray(&app),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

#[tauri::command]
#[specta::specta]
pub fn app_settings_set(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    settings: AppSettings,
) -> CommandResult<()> {
    settings::set(&db.conn(), tray::CLOSE_TO_TRAY_KEY, &settings.close_to_tray)?;
    let autolaunch = app.autolaunch();
    let result = if settings.autostart {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| AppError::Invalid(format!("autostart: {e}")))?;
    Ok(())
}
