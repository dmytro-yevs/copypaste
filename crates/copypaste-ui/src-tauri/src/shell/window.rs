//! Popover-style window behaviour.
//!
//! A menu-bar clipboard manager is a popover, not a document window: it appears
//! on a gesture, takes focus, and goes away when the user looks elsewhere. Three
//! behaviours make that true, and each one is a defect if it is missing.
//!
//! * **Closing hides.** The window is the app's only surface, so closing it
//!   must not quit — quitting is a tray decision (manifest 06, INV-36).
//! * **Losing focus hides.** A popover that stays up after you click elsewhere
//!   is a window, and an always-on-top window that will not go away is worse
//!   than no popover at all.
//! * **Showing focuses.** A window shown without focus swallows the first
//!   keystroke, which for a search-first UI is the whole interaction.
//!
//! # What is *not* here
//!
//! Positioning under the menu-bar item. Tauri's tray gives an icon rect, and
//! the arithmetic to place a window under it while keeping it on-screen across
//! multiple displays with different scale factors is real work that needs a
//! machine to test on. The window is centred instead, which is honest and
//! wrong-looking rather than subtly wrong; it is listed in ADR-0003 as
//! outstanding.

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// The one window. Named in `tauri.conf.json`.
pub const MAIN: &str = "main";

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(MAIN)
}

/// Show and focus, in that order.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    let _ = window.show();
    // Focus is what makes the search field live. Requested after `show`
    // because focusing a hidden window is a no-op on macOS.
    let _ = window.set_focus();
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

/// What the hotkey and the tray's Show/Hide item both do.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    // `is_visible` failing is not a reason to do nothing: treating an
    // unanswerable window as hidden means the gesture still shows it, which is
    // the recoverable direction.
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show(app);
    }
}

/// Window-event policy. Wired once in `crate::run`.
///
/// `Focused(false)` hides rather than closes so that the WebView keeps its
/// state — a popover that reloaded React on every dismissal would lose the
/// scroll position and the search text, and manifest 06's scroll-anchoring
/// requirements assume the list survives.
pub fn on_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        tauri::WindowEvent::Focused(false) => {
            let _ = window.hide();
        }
        _ => {}
    }
}
