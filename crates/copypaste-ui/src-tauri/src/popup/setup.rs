//! Window setup wiring: main-window close-to-tray, popup blur/vibrancy, and
//! macOS app-level activation-policy + vibrancy setup.

use tauri::Manager;

/// Wire the main window so closing it HIDES it to the tray instead of quitting
/// the whole app. This is the standard macOS menu-bar pattern and is what keeps
/// the app-owned daemon alive on a window close: only a real Quit (tray "Quit"
/// → `app.exit(0)`) terminates the process and triggers `stop_daemon`.
pub(crate) fn setup_main_window(app: &tauri::App) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let win_clone = win.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Prevent the close from propagating to app exit; hide instead.
            api.prevent_close();
            let _ = win_clone.hide();
        }
    });
}

/// Wire blur-handler and vibrancy onto a freshly-created popup window.
///
/// Called once from `toggle_popup` the first time the popup is lazy-created.
/// Extracted from the old `setup_popup_window` so the logic is reusable
/// regardless of who built the window.
///
/// M1: popup is no longer created at app launch; it is built on first hotkey
/// press.  `setup_popup_window` is therefore no longer called from setup.
pub(super) fn wire_popup(popup: &tauri::WebviewWindow) {
    // V-12 fix: hide on focus loss, but guard with is_visible() to prevent
    // double-activation when a JS-initiated hide (row click → invoke("hide_popup"))
    // fires concurrently with this blur event.  Without the guard the prior app
    // would be activated twice → focus flicker.  Also skip when a child/system
    // dialog (e.g. file picker) steals focus — those cause Focused(false) too,
    // and auto-dismissing the popup in that case is wrong.
    // We clone the popup handle (cheap Arc clone in Tauri 2) so the 'static
    // closure owns it without borrowing `popup`.
    let popup_for_blur = popup.clone();
    popup.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            // Skip if already hidden — avoids double hide_popup_internal call
            // when JS already called invoke("hide_popup") on the same dismiss.
            if !popup_for_blur.is_visible().unwrap_or(true) {
                return;
            }
            // hide_popup_internal requires an AppHandle; get it from the window.
            super::window::hide_popup_internal(popup_for_blur.app_handle());
        }
    });

    // Apply vibrancy on macOS.
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        let _ = apply_vibrancy(
            popup,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            Some(12.0),
        );
    }
}

// ---------------------------------------------------------------------------
// macOS setup
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub(crate) fn setup_macos(app: &tauri::App, allow_screenshots: bool) {
    use tauri::ActivationPolicy;
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

    // Regular policy so CopyPaste appears in the Cmd+Tab application switcher
    // (and shows a Dock icon). Accessory hides it from both Cmd+Tab and the Dock;
    // alt-tab presence on macOS is only available with Regular policy.
    // set_activation_policy on App requires &mut self; use the AppHandle variant (&self) instead.
    let _ = app
        .handle()
        .set_activation_policy(ActivationPolicy::Regular);
    if let Some(win) = app.get_webview_window("main") {
        let _ = apply_vibrancy(
            &win,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::Active),
            None,
        );

        // PG-25 / CopyPaste-13a3 / CopyPaste-6uy9:
        // Prevent screenshots and screen recordings from capturing clipboard
        // history UNLESS the user has explicitly enabled "Allow screenshots" in
        // Settings (allow_screenshots = true).  Default = false (protection ON).
        // Android parity: HistoryActivity.kt:224-227 sets FLAG_SECURE.
        // Non-fatal: log and continue on failure.
        if !allow_screenshots {
            if let Err(e) = win.set_content_protected(true) {
                tracing::warn!("PG-25: set_content_protected(true) failed: {e}");
            }
        } else {
            tracing::info!("CopyPaste-6uy9: content protection disabled — screenshots allowed by user preference");
        }
    }
}
