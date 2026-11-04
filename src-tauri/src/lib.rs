pub mod commands;
pub mod db;
pub mod error;
pub mod launcher;
pub mod metadata;
pub mod models;
pub mod secrets;
pub mod sources;
pub mod tracker;

use tauri::Manager;

use commands::app::AppPaths;

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,ludotheque_lib=debug"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

pub fn run() {
    init_logging();
    let builder = commands::specta_builder();

    #[cfg(debug_assertions)]
    commands::export_bindings();

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join(db::DB_FILE_NAME);
            let database = db::Db::open(&db_path)?;
            tracing::info!(path = %db_path.display(), "database ready");

            app.manage(database);
            app.manage(secrets::default_store()?);
            app.manage(AppPaths { data_dir, db_path });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ludothèque");
}
