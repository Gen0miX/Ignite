pub mod commands;
pub mod db;
pub mod error;
pub mod http;
pub mod launcher;
pub mod legacy;
pub mod library;
pub mod metadata;
pub mod models;
pub mod popup;
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

/// Debug builds only: `IGNITE_DATA_DIR` runs on another data folder (its own
/// database and tracker lock), e.g. for end-to-end checks next to a running
/// Ignite without touching the real library.
fn dev_data_dir() -> Option<std::path::PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os("IGNITE_DATA_DIR").map(std::path::PathBuf::from)
    } else {
        None
    }
}

pub fn run() {
    init_logging();
    let builder = commands::specta_builder();

    #[cfg(debug_assertions)]
    commands::export_bindings();

    // Before the window (WebView data) and the database open.
    let kept_legacy_dir = legacy::migrate();

    #[allow(unused_mut)]
    let mut app = tauri::Builder::default();
    // A second launch (shortcut, autostart) shows the running Ignite instead
    // of starting another one, which would track every session twice. Not in
    // dev, so `pnpm tauri dev` still runs next to the installed app (the
    // tracker lock then keeps a single tracker).
    #[cfg(not(debug_assertions))]
    {
        app = app.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }));
    }
    app.plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![tray::MINIMIZED_ARG]),
        ))
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let data_dir = match (dev_data_dir(), kept_legacy_dir) {
                (Some(dir), _) | (None, Some(dir)) => dir,
                (None, None) => app.path().app_data_dir()?,
            };
            let db_path = legacy::db_path(&data_dir);
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
            crate::sources::goldberg::spawn_watcher(app.handle().clone());
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
                // After the sync, so new games get their captures.
                commands::captures::scan_now(&handle);
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
