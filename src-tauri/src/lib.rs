pub mod commands;
pub mod db;
pub mod error;
pub mod http;
pub mod launcher;
pub mod library;
pub mod metadata;
pub mod models;
pub mod secrets;
pub mod sources;
pub mod tracker;
pub mod tray;

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![tray::MINIMIZED_ARG]),
        ))
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let data_dir = app.path().app_data_dir()?;
            let db_path = data_dir.join(db::DB_FILE_NAME);
            let database = db::Db::open(&db_path)?;
            tracing::info!(path = %db_path.display(), "database ready");

            app.manage(database);
            app.manage(secrets::default_store()?);
            // Serve cached artwork through `asset:` (only this folder).
            let artwork_dir = metadata::artwork::cache_root(&data_dir);
            std::fs::create_dir_all(&artwork_dir)?;
            app.asset_protocol_scope()
                .allow_directory(&artwork_dir, true)?;

            app.manage(AppPaths { data_dir, db_path });
            app.manage(commands::sources::SyncState::default());
            app.manage(tracker::TrackerState::default());
            tracker::spawn(app.handle().clone());
            if let Err(e) = tray::setup(app.handle()) {
                tracing::warn!(error = %e, "tray icon unavailable");
            }
            // Started with Windows: stay in the notification area.
            if !std::env::args().any(|a| a == tray::MINIMIZED_ARG) {
                tray::show_main(app.handle());
            }

            // Initial sync in the background: the window opens immediately and
            // the library refreshes when the `library-changed` event fires.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = commands::sources::run_sync(&handle) {
                    tracing::warn!(error = %e, "startup sync failed");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && tray::close_to_tray(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Ignite")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Write sessions still open before quitting.
                if let Some(state) = app.try_state::<tracker::TrackerState>() {
                    state.shutdown();
                }
            }
        });
}
