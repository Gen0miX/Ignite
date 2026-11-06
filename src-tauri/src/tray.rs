//! Notification-area icon: live timer of the game being played, quick
//! actions, and "close to tray" so the tracker keeps running.

use std::time::Duration;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::db::repo::{games, settings};
use crate::db::Db;
use crate::tracker::TrackerState;

pub const CLOSE_TO_TRAY_KEY: &str = "app.close_to_tray";
const TRAY_ID: &str = "main";
/// Command-line flag passed by the "start with Windows" entry.
pub const MINIMIZED_ARG: &str = "--minimized";

/// "1 h 05" / "12 min".
pub fn format_duration(secs: u32) -> String {
    let minutes = secs / 60;
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {:02}", minutes / 60, minutes % 60)
    }
}

/// Line shown in the tray menu and tooltip, `None` when nothing is played.
pub fn now_playing_label(playing: &[(String, u32)]) -> Option<String> {
    match playing {
        [] => None,
        [(title, secs)] => Some(format!("En jeu : {title} · {}", format_duration(*secs))),
        [(title, secs), rest @ ..] => Some(format!(
            "En jeu : {title} · {} (+{})",
            format_duration(*secs),
            rest.len()
        )),
    }
}

pub fn close_to_tray(app: &AppHandle) -> bool {
    app.try_state::<Db>()
        .and_then(|db| {
            settings::get::<bool>(&db.conn(), CLOSE_TO_TRAY_KEY)
                .ok()
                .flatten()
        })
        .unwrap_or(true)
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn playing(app: &AppHandle) -> Vec<(String, u32)> {
    let Some(state) = app.try_state::<TrackerState>() else {
        return vec![];
    };
    let active = state.active();
    if active.is_empty() {
        return vec![];
    }
    let db = app.state::<Db>();
    let conn = db.conn();
    active
        .into_iter()
        .map(|a| {
            let title = games::find(&conn, &a.game_id)
                .ok()
                .flatten()
                .map(|g| g.title)
                .unwrap_or_else(|| "?".into());
            (title, a.active_seconds)
        })
        .collect()
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let now = MenuItem::with_id(app, "now", "Aucun jeu en cours", false, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Afficher Ludothèque", true, None::<&str>)?;
    let couch = MenuItem::with_id(app, "couch", "Mode Canapé", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu: Menu<Wry> = Menu::with_items(
        app,
        &[
            &now,
            &PredefinedMenuItem::separator(app)?,
            &show,
            &couch,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Ludothèque")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "couch" => {
                show_main(app);
                let _ = app.emit("open-couch", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;

    // Live timer: refreshed every 15 s (the tracker polls every 3 s).
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut last: Option<String> = None;
        loop {
            let label = now_playing_label(&playing(&handle));
            if label != last {
                let tooltip = match &label {
                    Some(l) => format!("Ludothèque — {l}"),
                    None => "Ludothèque".into(),
                };
                let _ = tray.set_tooltip(Some(&tooltip));
                let _ = now.set_text(label.as_deref().unwrap_or("Aucun jeu en cours"));
                last = label;
            }
            std::thread::sleep(Duration::from_secs(15));
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_the_game_being_played() {
        assert_eq!(now_playing_label(&[]), None);
        assert_eq!(
            now_playing_label(&[("Hades II".into(), 3_900)]).as_deref(),
            Some("En jeu : Hades II · 1 h 05")
        );
        assert_eq!(
            now_playing_label(&[("Tetris".into(), 720), ("RetroArch".into(), 30)]).as_deref(),
            Some("En jeu : Tetris · 12 min (+1)")
        );
    }
}
