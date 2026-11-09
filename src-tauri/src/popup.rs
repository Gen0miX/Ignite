//! Ignite-styled notifications: a small borderless window in the corner of
//! the screen, always on top, that never takes the focus nor the clicks (the
//! game keeps both). Windows toasts cannot follow the app's theme.
//!
//! Popups are queued here. The first one opens a window (`notification.html`)
//! that pulls them one by one with `popup_next`; once the queue is empty it
//! calls `popup_idle`, which closes the window under the same lock, so a popup
//! pushed meanwhile either is picked up or opens a new window.
//!
//! Not visible over a game in true exclusive fullscreen (no outside window
//! is): for Goldberg games, the in-game overlay covers that case.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};

/// Window size in logical pixels (the card plus room for its shadow).
const WIDTH: f64 = 400.0;
const HEIGHT: f64 = 150.0;
/// Gap with the screen edges, in logical pixels.
const MARGIN: f64 = 12.0;
/// Popups waiting beyond this are dropped (a burst stays readable).
const MAX_QUEUED: usize = 12;
const LABEL_PREFIX: &str = "popup-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PopupKind {
    /// One achievement unlocked.
    Achievement,
    /// Several achievements at once (`count`), e.g. found in existing saves.
    Achievements,
    /// A play session ended (`description` holds the duration text).
    Session,
    /// Sample shown by the "test the notification" button.
    Test,
}

/// What the popup shows. Texts that depend on the kind (« Succès débloqué »…)
/// come from the frontend's i18n.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Popup {
    pub kind: PopupKind,
    pub title: String,
    pub description: Option<String>,
    pub game: Option<String>,
    /// Local file path or `https` URL.
    pub image: Option<String>,
    pub count: Option<u32>,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Popup>,
    /// Label of the open popup window.
    active: Option<String>,
    next_id: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    queue: VecDeque::new(),
    active: None,
    next_id: 0,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Queues a popup and opens the window if none is showing. Never call it from
/// a synchronous command: building a webview there deadlocks on Windows.
pub fn show(app: &AppHandle, popup: Popup) -> AppResult<()> {
    if let Some(image) = popup.image.as_deref() {
        allow_image(app, image);
    }
    let mut st = state();
    if st.queue.len() >= MAX_QUEUED {
        tracing::debug!("popup queue full, dropping one");
        return Ok(());
    }
    st.queue.push_back(popup);
    let open = st
        .active
        .as_deref()
        .is_some_and(|label| app.get_webview_window(label).is_some());
    if open {
        return Ok(());
    }
    st.next_id += 1;
    let label = format!("{LABEL_PREFIX}{}", st.next_id);
    match open_window(app, &label) {
        Ok(()) => {
            st.active = Some(label);
            Ok(())
        }
        Err(e) => {
            st.queue.clear();
            st.active = None;
            Err(e)
        }
    }
}

/// Lets the webview load a local icon through `asset:`.
fn allow_image(app: &AppHandle, image: &str) {
    if image.starts_with("http://") || image.starts_with("https://") {
        return;
    }
    if let Some(dir) = Path::new(image).parent() {
        let _ = app.asset_protocol_scope().allow_directory(dir, false);
    }
}

fn open_window(app: &AppHandle, label: &str) -> AppResult<()> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("notification.html".into()))
        .title("Ignite")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|e| AppError::Invalid(format!("popup window: {e}")))?;
    place(&window);
    // Clicks go through to the game.
    let _ = window.set_ignore_cursor_events(true);
    Ok(())
}

/// Bottom-right corner of the primary monitor's work area (above the taskbar).
fn place(window: &WebviewWindow) {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (x, y) = corner(
        (area.position.x, area.position.y),
        (area.size.width, area.size.height),
        scale,
    );
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Physical top-left position of the popup in a work area.
fn corner(origin: (i32, i32), size: (u32, u32), scale: f64) -> (i32, i32) {
    let w = ((WIDTH + MARGIN) * scale).round() as i32;
    let h = ((HEIGHT + MARGIN) * scale).round() as i32;
    let right = origin.0 + i32::try_from(size.0).unwrap_or(i32::MAX);
    let bottom = origin.1 + i32::try_from(size.1).unwrap_or(i32::MAX);
    ((right - w).max(origin.0), (bottom - h).max(origin.1))
}

/// Next popup to show, if any.
#[tauri::command]
#[specta::specta]
pub fn popup_next() -> Option<Popup> {
    state().queue.pop_front()
}

/// Called by the popup window once it has nothing left to show. Returns
/// `true` when popups arrived meanwhile (keep going); otherwise the window
/// is closed.
#[tauri::command]
#[specta::specta]
pub async fn popup_idle(window: WebviewWindow) -> bool {
    let mut st = state();
    if !st.queue.is_empty() {
        return true;
    }
    if st.active.as_deref() == Some(window.label()) {
        st.active = None;
    }
    drop(st);
    let _ = window.destroy();
    false
}

/// The popup has painted its first frame: show the window without
/// activating it.
#[tauri::command]
#[specta::specta]
pub fn popup_ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_always_on_top(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_sits_above_the_taskbar_on_the_right() {
        // 1920×1040 work area (taskbar 40 px at the bottom), 100 %.
        assert_eq!(corner((0, 0), (1920, 1040), 1.0), (1508, 878));
        // 150 %: sizes are scaled.
        assert_eq!(corner((0, 0), (2880, 1560), 1.5), (2262, 1317));
        // Secondary origin (work area not at 0,0).
        assert_eq!(corner((-1920, 0), (1920, 1040), 1.0), (-412, 878));
    }

    #[test]
    fn corner_never_leaves_a_tiny_area() {
        assert_eq!(corner((10, 20), (100, 50), 1.0), (10, 20));
    }
}
