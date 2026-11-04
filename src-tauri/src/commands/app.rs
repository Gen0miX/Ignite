use std::path::PathBuf;

use serde::Serialize;

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
