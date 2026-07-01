//! Popup show/hide/toggle + the popup's logical dimensions.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::{AllowScreenshots, CurrentPopupPosition};

#[cfg(target_os = "macos")]
use super::state::PriorApp;

// ---------------------------------------------------------------------------
// Popup dimensions
// ---------------------------------------------------------------------------

/// Logical popup dimensions (must match tauri.conf.json).
pub(super) const POPUP_W_LOGICAL: f64 = 403.0; // v0.5.3: matches tauri.conf.json popup width (was 504)
pub(super) const POPUP_H_LOGICAL: f64 = 624.0; // v0.5.3: 1.2× enlargement (was 520)

// ---------------------------------------------------------------------------
// Popup show/hide/position
// ---------------------------------------------------------------------------

/// Shared internal implementation for hiding the popup without surfacing the
/// main window.  Both the `hide_popup` Tauri command and the `toggle_popup`
/// close-branch call this so the macOS prior-app activation logic is never
/// duplicated or skipped (V-10 fix: toggle_popup was calling `popup.hide()`
/// directly, bypassing this path).
///
/// V-11 fix: when no prior app is recorded (e.g. first-ever popup open before
/// the user has switched away to any external app), temporarily switch to the
/// Accessory activation policy before hiding so macOS does not promote the
/// main window.  The policy is restored to Regular immediately after — the
/// switch is invisible to the user because the popup is still visible during
/// the policy change.
pub(crate) fn hide_popup_internal(handle: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let bundle_id: Option<String> = handle
            .try_state::<PriorApp>()
            .and_then(|s| s.0.lock().ok().map(|g| g.clone()))
            .flatten();

        if let Some(ref bid) = bundle_id {
            // Activate the prior external app so macOS hands focus there
            // instead of to our main window (D7 fix).
            super::focus::activate_app_by_bundle_id(bid);
        } else {
            // V-11: No prior app recorded (first launch before any external
            // app has been focused, or Esc pressed immediately).  Temporarily
            // set Accessory policy so the OS does not auto-promote the main
            // window when the popup disappears, then restore Regular so the
            // Dock icon and Cmd+Tab entry remain visible.
            use tauri::ActivationPolicy;
            let _ = handle.set_activation_policy(ActivationPolicy::Accessory);
            if let Some(popup) = handle.get_webview_window("popup") {
                let _ = popup.hide();
                // M1: free JS heap (image LRU cache + items list) after hide
                // so the idle WebView holds minimal memory.  The warm WebView
                // stays alive — only the JS heap is freed.
                let _ =
                    popup.eval("window.__copypasteFreeMemory && window.__copypasteFreeMemory()");
            }
            let _ = handle.set_activation_policy(ActivationPolicy::Regular);
            return;
        }
    }

    if let Some(popup) = handle.get_webview_window("popup") {
        let _ = popup.hide();
        // M1: free JS heap (image LRU cache + items list) after hide so the
        // idle WebView holds minimal memory.  On the next show, the existing
        // onFocusChanged → refresh() re-populates the list.
        let _ = popup.eval("window.__copypasteFreeMemory && window.__copypasteFreeMemory()");
    }
}

/// Hide the popup window without surfacing the main window.
///
/// On macOS, simply calling `win.hide()` from JS causes the OS to promote the
/// next window of the same Regular-policy app to the front — which is our main
/// window.  This command first activates the prior (external) app so that macOS
/// hands focus there instead of to our main window, then hides the popup.
/// This is the correct hide path for Esc, blur, and row-click dismiss actions.
/// Delegates to `hide_popup_internal` so `toggle_popup` shares the same path
/// (V-10 fix).
#[tauri::command]
pub(crate) fn hide_popup(handle: tauri::AppHandle) {
    hide_popup_internal(&handle);
}

/// Toggle (show or hide) the quick-paste popup using the configured position mode.
///
/// M1: Lazy-create the popup WebView on the first toggle instead of at app
/// launch — saves ~84 MB of idle RSS (full WKWebView process + JS heap that
/// was previously sitting warm even when the popup was never opened).
/// The warm path (window already created) is unaffected: only the JS heap is
/// freed on hide (via `window.__copypasteFreeMemory`), not the WebView itself,
/// so show-latency stays instant.
pub(crate) fn toggle_popup(handle: &tauri::AppHandle) {
    // M1: Get existing window or lazy-create it on first invocation.
    let popup = match handle.get_webview_window("popup") {
        Some(w) => w,
        None => {
            // First ever toggle — build the popup WebView now.
            match WebviewWindowBuilder::new(handle, "popup", WebviewUrl::App("popup.html".into()))
                .title("CopyPaste Quick Paste")
                .inner_size(POPUP_W_LOGICAL, POPUP_H_LOGICAL)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .build()
            {
                Ok(w) => {
                    // Wire blur-handler + vibrancy the first time we build.
                    // Infallible: we just built the window above.
                    super::setup::wire_popup(&w);
                    // CopyPaste-c27b / CopyPaste-6uy9: exclude the popup from
                    // screen capture unless the user has enabled "Allow screenshots".
                    // Maps to macOS NSWindowSharingNone. Non-fatal.
                    let allow = handle
                        .try_state::<AllowScreenshots>()
                        .map(|s| *s.0.lock().expect("mutex poisoned"))
                        .unwrap_or(false);
                    if !allow {
                        if let Err(e) = w.set_content_protected(true) {
                            tracing::warn!(
                                "CopyPaste-c27b: popup set_content_protected(true) failed: {e}"
                            );
                        }
                    }
                    w
                }
                Err(e) => {
                    tracing::error!("toggle_popup: failed to create popup window: {e}");
                    return;
                }
            }
        }
    };

    // If the popup is already visible, hide it via the shared internal helper
    // so the macOS prior-app activation runs (V-10 fix: was calling
    // popup.hide() directly, which skipped activation and surfaced main window).
    let is_visible = popup.is_visible().unwrap_or(false);
    if is_visible {
        hide_popup_internal(handle);
        return;
    }

    // Read the current position mode from managed state.
    let mode = handle
        .try_state::<CurrentPopupPosition>()
        .map(|s| s.0.lock().expect("mutex poisoned").clone())
        .unwrap_or_default();

    super::position::position_popup(&popup, &mode);

    // Fix #3: record which app was frontmost BEFORE we bring our popup to focus,
    // so paste_to_frontmost can return focus there (not to the main window).
    #[cfg(target_os = "macos")]
    {
        if let Some(state) = handle.try_state::<PriorApp>() {
            let bundle_id = super::focus::frontmost_bundle_id();
            let mut guard = state.0.lock().expect("mutex poisoned");
            *guard = bundle_id;
        }
    }

    let _ = popup.show();
    let _ = popup.set_focus();
}
