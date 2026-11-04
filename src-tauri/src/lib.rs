pub mod db;
pub mod error;
pub mod models;
pub mod secrets;

use tauri::Manager;

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,ludotheque_lib=debug"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

pub fn run() {
    init_logging();
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let path = dir.join(db::DB_FILE_NAME);
            let database = db::Db::open(&path)?;
            tracing::info!(path = %path.display(), "database ready");
            app.manage(database);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ludothèque");
}
