//! Popup window management — create, show, hide, position — plus macOS
//! helpers for focus tracking, CGEventTap, and synthetic paste events.

// `Mutex` backs only the macOS-only managed state (`PriorApp`/`TapActive`), so
// gate the import to macOS — otherwise it is an unused import on Linux (-D warnings).
#[cfg(target_os = "macos")]
use std::sync::Mutex;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::{AllowScreenshots, CurrentPopupPosition, PopupPosition};

// ---------------------------------------------------------------------------
// macOS-only managed state
// ---------------------------------------------------------------------------

/// Bundle ID (or process identifier as fallback) of the app that was
/// frontmost when the popup was last shown.  Used to restore focus after
/// the user picks an item.
#[cfg(target_os = "macos")]
pub(crate) struct PriorApp(pub(crate) Mutex<Option<String>>);

/// Whether the CGEventTap is active (Accessibility permission was granted and
/// the tap was successfully installed).
#[cfg(target_os = "macos")]
pub(crate) struct TapActive(pub(crate) Mutex<bool>);

// ---------------------------------------------------------------------------
// Popup dimensions
// ---------------------------------------------------------------------------

/// Logical popup dimensions (must match tauri.conf.json).
const POPUP_W_LOGICAL: f64 = 403.0; // v0.5.3: matches tauri.conf.json popup width (was 504)
const POPUP_H_LOGICAL: f64 = 624.0; // v0.5.3: 1.2× enlargement (was 520)

// ---------------------------------------------------------------------------
// macOS helpers
// ---------------------------------------------------------------------------

/// Return the bundle identifier of the currently frontmost application,
/// or `None` if it cannot be determined.
#[cfg(target_os = "macos")]
pub(crate) fn frontmost_bundle_id() -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    // SAFETY: ObjC calls are safe when the types and selectors are correct;
    // objc2-app-kit guarantees these bindings.
    unsafe {
        let ws = NSWorkspace::sharedWorkspace();
        let app = ws.frontmostApplication()?;
        // bundleIdentifier returns Option<Retained<NSString>>; Display impl calls
        // autoreleasepool_leaking internally so we can call .to_string() safely.
        let nsstring = app.bundleIdentifier()?;
        Some(nsstring.to_string())
    }
}

/// Poll until the app identified by `target_bundle_id` is the frontmost
/// application, or until `max_iter` iterations have been exhausted.
///
/// `probe` is called on each iteration and should return the current frontmost
/// bundle ID (or `None` if indeterminate).  Between probes the caller sleeps
/// for `interval`.
///
/// Returns `true` when a probe matches `target_bundle_id`, `false` on timeout.
///
/// # Why poll instead of a fixed sleep?
/// A fixed 80 ms sleep is racy: on a slow or heavily-loaded system the prior
/// app may not have become frontmost yet, so the synthesised Cmd+V fires into
/// the wrong window and the paste is silently dropped.  Polling terminates as
/// soon as the OS confirms the transition, making the happy path faster while
/// the bounded timeout (≈ 300 ms with default params) prevents an indefinite hang
/// on pathological cases where activation never completes.
///
/// This function is extracted so it can be unit-tested without macOS ObjC bindings.
// Only the macOS activation path calls this; on other platforms the sole caller
// is `#[cfg(target_os = "macos")]`, so gate the fn (plus `test` for the
// platform-independent unit test) to avoid a dead_code error under -D warnings.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn poll_until_frontmost(
    target_bundle_id: &str,
    mut probe: impl FnMut() -> Option<String>,
    max_iter: u32,
    interval: std::time::Duration,
) -> bool {
    for _ in 0..max_iter {
        if probe().as_deref() == Some(target_bundle_id) {
            return true;
        }
        std::thread::sleep(interval);
    }
    false
}

/// Activate the app with the given bundle identifier using NSRunningApplication.
#[cfg(target_os = "macos")]
pub(crate) fn activate_app_by_bundle_id(bundle_id: &str) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    use objc2_foundation::NSString;

    // NSApplicationActivateIgnoringOtherApps is deprecated in macOS 14 but
    // remains the correct approach for cross-process activation of a specific
    // app by bundle ID; there is no non-deprecated replacement that provides
    // the same behaviour for background processes.
    #[allow(deprecated)]
    unsafe {
        let bid = NSString::from_str(bundle_id);
        let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&bid);
        if let Some(app) = apps.firstObject() {
            let _ = app.activateWithOptions(
                NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
            );
        }
    }
}

/// Try to install the CGEventTap.  Silently logs and falls back to the
/// plugin-based shortcut if Accessibility is not granted.
#[cfg(target_os = "macos")]
pub(crate) fn try_install_event_tap(handle: &tauri::AppHandle, accel: &str) {
    let tap_state = handle.try_state::<TapActive>();
    // Already installed?
    if let Some(ref ts) = tap_state {
        if *ts.0.lock().expect("mutex poisoned") {
            // Just update the accelerator.
            crate::event_tap::update_tap_shortcut(accel);
            return;
        }
    }

    let handle_clone = handle.clone();
    match crate::event_tap::install(accel, move || {
        toggle_popup(&handle_clone);
    }) {
        Ok(()) => {
            tracing::info!("CGEventTap installed — OS-reserved shortcuts can now be overridden");
            if let Some(ts) = tap_state {
                *ts.0.lock().expect("mutex poisoned") = true;
            }
        }
        Err(e) => {
            tracing::info!("CGEventTap not installed ({e}); using plugin-global-shortcut fallback");
        }
    }
}

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
            activate_app_by_bundle_id(bid);
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
                    wire_popup(&w);
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

    position_popup(&popup, &mode);

    // Fix #3: record which app was frontmost BEFORE we bring our popup to focus,
    // so paste_to_frontmost can return focus there (not to the main window).
    #[cfg(target_os = "macos")]
    {
        if let Some(state) = handle.try_state::<PriorApp>() {
            let bundle_id = frontmost_bundle_id();
            let mut guard = state.0.lock().expect("mutex poisoned");
            *guard = bundle_id;
        }
    }

    let _ = popup.show();
    let _ = popup.set_focus();
}

// ---------------------------------------------------------------------------
// Popup positioning — multi-mode, clamped to visible screen frame
// ---------------------------------------------------------------------------

/// Position the popup window according to `mode`, clamping it onto the visible
/// screen frame so it never appears partially off-screen.
///
/// All arithmetic is in physical pixels:
///   - `cursor_position()` / `monitor.position()` / `monitor.size()` are physical px
///   - `monitor.scale_factor()` converts logical popup dims to physical px
///   - `set_position(PhysicalPosition)` places the window in physical px
fn position_popup(win: &tauri::WebviewWindow, mode: &PopupPosition) {
    let monitors = win.available_monitors().unwrap_or_default();
    let primary = win.primary_monitor().ok().flatten();

    // Resolve the target monitor + raw (x, y) before clamping.
    let (target_monitor, raw_x, raw_y): (Option<tauri::Monitor>, i32, i32) = match mode {
        PopupPosition::Cursor => {
            // cursor_position() returns physical pixels.
            let cursor: tauri::PhysicalPosition<i32> = win
                .cursor_position()
                .map(|p| tauri::PhysicalPosition {
                    x: p.x as i32,
                    y: p.y as i32,
                })
                .unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });

            // Small offset so the popup doesn't sit right on the cursor tip.
            const OFFSET: i32 = 8;
            let rx = cursor.x + OFFSET;
            let ry = cursor.y + OFFSET;

            // Find the monitor whose physical bounds contain the cursor.
            // Iterate all monitors to handle negative coords on secondary displays.
            let mon = monitors
                .iter()
                .find(|m| {
                    let pos = m.position();
                    let size = m.size();
                    let (mx, my) = (pos.x, pos.y);
                    let (mw, mh) = (size.width as i32, size.height as i32);
                    cursor.x >= mx && cursor.x < mx + mw && cursor.y >= my && cursor.y < my + mh
                })
                .cloned()
                .or_else(|| primary.clone());

            (mon, rx, ry)
        }

        PopupPosition::Center => {
            // Center on the primary monitor (or first available).
            let mon = primary.clone().or_else(|| monitors.first().cloned());
            let (rx, ry) = if let Some(ref m) = mon {
                let pos = m.position();
                let size = m.size();
                let scale = m.scale_factor();
                let popup_w = (POPUP_W_LOGICAL * scale) as i32;
                let popup_h = (POPUP_H_LOGICAL * scale) as i32;
                let cx = pos.x + (size.width as i32 - popup_w) / 2;
                let cy = pos.y + (size.height as i32 - popup_h) / 2;
                (cx, cy)
            } else {
                (0, 0)
            };
            (mon, rx, ry)
        }

        PopupPosition::Menubar => {
            // Place below the tray / menu-bar area — top-right of the primary monitor.
            // macOS menu bar height is 24 pt logical; add a 4 pt gap.
            const MENUBAR_HEIGHT_LOGICAL: f64 = 24.0;
            const GAP_LOGICAL: f64 = 4.0;

            let mon = primary.clone().or_else(|| monitors.first().cloned());
            let (rx, ry) = if let Some(ref m) = mon {
                let pos = m.position();
                let size = m.size();
                let scale = m.scale_factor();
                let popup_w = (POPUP_W_LOGICAL * scale) as i32;
                let bar_h = ((MENUBAR_HEIGHT_LOGICAL + GAP_LOGICAL) * scale) as i32;
                // Align right edge with right edge of screen, 8 px inset.
                const RIGHT_INSET_LOGICAL: f64 = 8.0;
                let right_inset = (RIGHT_INSET_LOGICAL * scale) as i32;
                let rx = pos.x + size.width as i32 - popup_w - right_inset;
                let ry = pos.y + bar_h;
                (rx, ry)
            } else {
                (0, 0)
            };
            (mon, rx, ry)
        }
    };

    // Clamp raw position onto the monitor's frame so the popup is always fully visible.
    let (x, y) = if let Some(monitor) = target_monitor {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = monitor.scale_factor();

        let popup_w = (POPUP_W_LOGICAL * scale) as i32;
        let popup_h = (POPUP_H_LOGICAL * scale) as i32;

        let mon_x = pos.x;
        let mon_y = pos.y;
        let mon_w = size.width as i32;
        let mon_h = size.height as i32;

        let max_x = mon_x + mon_w - popup_w;
        let max_y = mon_y + mon_h - popup_h;

        (
            raw_x.clamp(mon_x, max_x.max(mon_x)),
            raw_y.clamp(mon_y, max_y.max(mon_y)),
        )
    } else {
        (raw_x, raw_y)
    };

    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

// ---------------------------------------------------------------------------
// Window setup helpers
// ---------------------------------------------------------------------------

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
fn wire_popup(popup: &tauri::WebviewWindow) {
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
            hide_popup_internal(popup_for_blur.app_handle());
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

// ---------------------------------------------------------------------------
// Tauri commands — window management
// ---------------------------------------------------------------------------

/// Show the main CopyPaste window.
pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Bring the main CopyPaste window to the foreground.
///
/// Called by the React layer when an incoming pairing request (role=responder,
/// state=awaiting_sas) is detected so the SAS confirmation modal is visible
/// to the user immediately, without requiring them to open the app manually.
#[tauri::command]
pub(crate) fn focus_main_window(handle: tauri::AppHandle) {
    show_main(&handle);
}

/// Activate the previously-focused application (restoring focus) and then
/// synthesise a Cmd+V paste event so the clipboard content lands in the
/// target app.  Call this after `api.copyItem` and before hiding the popup.
#[tauri::command]
pub(crate) fn paste_to_frontmost(handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::{CGEvent, CGEventFlags, KeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        use std::thread;
        use std::time::Duration;

        // Read the saved prior-app bundle ID.
        let bundle_id: Option<String> = handle
            .try_state::<PriorApp>()
            .and_then(|s| s.0.lock().ok().map(|g| g.clone()))
            .flatten();

        // Activate the prior app, then wait for it to become frontmost before
        // synthesising Cmd+V.  The wait uses a bounded polling loop rather than
        // a fixed sleep so that:
        //   • On fast systems the paste fires as soon as the OS confirms the
        //     transition (typically 10–30 ms), not after a blind 80 ms delay.
        //   • On slow or loaded systems the old 80 ms was racy — paste landed
        //     in the wrong window.  The poll backs off up to 300 ms total
        //     (37 × 8 ms) then proceeds unconditionally so we never hang.
        //
        // CopyPaste-78hg fix: replaced fixed 80 ms sleep.
        thread::spawn(move || {
            // Activate the prior app by bundle ID (best effort).
            if let Some(ref bid) = bundle_id {
                activate_app_by_bundle_id(bid);
            }

            // Poll until the prior app is frontmost or the timeout expires.
            // 37 iterations × 8 ms ≈ 296 ms maximum wait.
            // When no prior-app bundle is known (popup opened but no external
            // app ever focused), skip the poll and proceed after a minimal wait
            // so the synthetic Cmd+V still fires even if it may land nowhere.
            const MAX_ITER: u32 = 37;
            const INTERVAL: Duration = Duration::from_millis(8);
            if let Some(ref bid) = bundle_id {
                let bid_clone = bid.clone();
                let matched = poll_until_frontmost(bid, frontmost_bundle_id, MAX_ITER, INTERVAL);
                if !matched {
                    tracing::debug!(
                        "paste_to_frontmost: '{}' did not become frontmost within \
                         {}ms; proceeding with Cmd+V anyway",
                        bid_clone,
                        MAX_ITER * INTERVAL.as_millis() as u32,
                    );
                }
            } else {
                // No prior app recorded; wait one interval before firing.
                thread::sleep(INTERVAL);
            }

            // Synthesise Cmd+V (key down + key up).
            //
            // CGEvent::new_keyboard_event returns Err only in pathological cases
            // (e.g. event source exhaustion). This runs on a detached background
            // thread, so a panic here would abort the process silently from the
            // user's point of view — handle the error and log instead.
            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                let v_kc = KeyCode::ANSI_V;
                match (
                    CGEvent::new_keyboard_event(source.clone(), v_kc, true),
                    CGEvent::new_keyboard_event(source, v_kc, false),
                ) {
                    (Ok(kd), Ok(ku)) => {
                        kd.set_flags(CGEventFlags::CGEventFlagCommand);
                        kd.post(core_graphics::event::CGEventTapLocation::Session);
                        ku.set_flags(CGEventFlags::CGEventFlagCommand);
                        ku.post(core_graphics::event::CGEventTapLocation::Session);
                    }
                    _ => {
                        tracing::warn!(
                            "paste_to_frontmost: failed to synthesise Cmd+V keyboard event"
                        );
                    }
                }
            } else {
                tracing::warn!("paste_to_frontmost: failed to create CGEventSource");
            }
        });
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = handle;
        Ok(())
    }
}

/// Write `text` as plain UTF-8 to the system clipboard (strips all rich
/// formatting / MIME context), then activate the prior app and synthesise
/// Cmd+V — the Option+Enter "paste as plain text" shortcut (F1).
///
/// The caller is responsible for hiding the popup first so the prior app
/// receives focus before `paste_to_frontmost` fires Cmd+V.
///
/// On non-macOS this is a no-op (returns Ok).
#[tauri::command]
pub(crate) fn paste_plain_text(text: String, handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
        use objc2_foundation::NSString;

        // Write plain text to the general pasteboard, clearing all prior types.
        //
        // SAFETY: NSPasteboard / NSString ObjC bindings are correct; these calls
        // are safe on any thread that has an autorelease pool. Tauri command
        // handlers run on the Tokio runtime which provides an autorelease pool on
        // macOS, so this is safe here.
        unsafe {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            let ns_text = NSString::from_str(&text);
            // setString_forType returns bool — false means the write was rejected
            // (e.g. pasteboard is owned by another process). Log and continue; the
            // Cmd+V will paste whatever was on the clipboard before (graceful
            // degradation).
            let ok = pb.setString_forType(&ns_text, NSPasteboardTypeString);
            if !ok {
                tracing::warn!("paste_plain_text: NSPasteboard setString_forType returned false");
            }
        }

        // Reuse the existing paste-to-frontmost logic (activate prior app + Cmd+V).
        paste_to_frontmost(handle)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, handle);
        Ok(())
    }
}

/// Play a soft system sound (NSSound "Tink") after a successful copy.
///
/// Maccy parity: Maccy plays "Funk" or "Pop" depending on version; we use
/// "Tink" because it is shorter and less intrusive. The sound plays on the
/// main run-loop via `[NSSound play]` which is non-blocking from the Rust
/// perspective. Any failure (sound file missing, audio device unavailable) is
/// silently ignored so it never disrupts the copy flow.
///
/// The command is cross-platform safe: on non-macOS it is a no-op.
#[tauri::command]
pub(crate) fn play_copy_sound() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSSound;
        use objc2_foundation::NSString;

        // SAFETY: NSSound and NSString bindings are correct; ObjC calls are
        // safe when invoked from a thread that has an autorelease pool. Tauri
        // command handlers run on the Tokio runtime, which drives an ObjC
        // autorelease pool on macOS — so this is safe here.
        unsafe {
            let name = NSString::from_str("Tink");
            if let Some(sound) = NSSound::soundNamed(&name) {
                // play returns bool; ignore the result — best-effort only.
                let _ = sound.play();
            } else {
                tracing::debug!("play_copy_sound: NSSound 'Tink' not found (non-fatal)");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands — Accessibility + CGEventTap (macOS)
// ---------------------------------------------------------------------------

/// Returns `true` when the Accessibility permission is granted.
/// On non-macOS platforms this always returns `true` (no permission needed).
#[tauri::command]
pub(crate) fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        crate::event_tap::accessibility_granted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open the System Settings Accessibility pane (macOS) and attempt to
/// (re-)install the CGEventTap if permission was just granted.
/// On non-macOS this is a no-op.
#[tauri::command]
pub(crate) fn request_accessibility_permission(handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        crate::event_tap::open_accessibility_settings();
        // The user may already have granted permission before clicking;
        // try installing the tap in the background.
        let shortcut = {
            let state: tauri::State<crate::config::CurrentShortcut> = handle.state();
            let s = state.0.lock().expect("mutex poisoned").clone();
            s
        };
        try_install_event_tap(&handle, &shortcut);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// Synchronise the macOS native NSWindow appearance with the CSS theme so that
/// `NSVisualEffectView` (vibrancy) renders the correct glass tint.
///
/// Without this, the sidebar NSVisualEffectView uses `effectiveAppearance`
/// inherited from the NSApplication — which follows the OS dark/light setting,
/// NOT the user's in-app theme choice.  Calling `[win setAppearance:]` pins the
/// window to the desired variant regardless of the OS preference.
///
/// Accepted values for `appearance`:
///   "light" → NSAppearanceNameAqua
///   "dark"  → NSAppearanceNameDarkAqua
///   anything else → no-op (leaves the window at OS default)
///
/// On non-macOS this command is a no-op so the invoke call is safe everywhere.
/// In browser/mock mode (`HAS_TAURI` is false in the frontend) this is never
/// called at all.
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(crate) fn set_native_appearance(appearance: String, handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2_app_kit::{
            NSAppearance, NSAppearanceName, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSView,
        };
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let Some(win) = handle.get_webview_window("main") else {
            return;
        };

        // Resolve the target appearance name constant.
        // NSAppearanceNameAqua/DarkAqua are `&'static NSAppearanceName` extern
        // statics; reading an extern static is unsafe.
        // SAFETY: these are immutable Apple-provided string constants — reading
        // the static reference has no preconditions and no data races.
        let appearance_name: &NSAppearanceName = unsafe {
            match appearance.as_str() {
                "light" => NSAppearanceNameAqua,
                "dark" => NSAppearanceNameDarkAqua,
                // Unknown value — leave the window at its OS-default appearance.
                _ => return,
            }
        };

        // Look up the NSAppearance by name.
        let Some(ns_appearance) = NSAppearance::appearanceNamed(appearance_name) else {
            tracing::warn!(
                "set_native_appearance: NSAppearance::appearanceNamed({appearance:?}) returned nil"
            );
            return;
        };

        // Obtain the raw AppKit window handle.  Tauri's WebviewWindow implements
        // HasWindowHandle; on macOS the inner handle is AppKit (ns_view pointer).
        let raw = match win.window_handle() {
            Ok(h) => h.as_raw(),
            Err(e) => {
                tracing::warn!("set_native_appearance: window_handle failed: {e}");
                return;
            }
        };
        let RawWindowHandle::AppKit(appkit_handle) = raw else {
            return;
        };

        // SAFETY: appkit_handle.ns_view is a valid NSView* for the lifetime of
        // the window.  We access it only on the main thread (Tauri command
        // handlers on macOS run on the main thread).  The cast to &NSView is safe
        // because objc2-app-kit 0.2 NSView is repr(C) / ABI-compatible.
        unsafe {
            let ns_view: &NSView = appkit_handle.ns_view.cast().as_ref();
            if let Some(ns_window) = ns_view.window() {
                // setAppearance: is declared on NSWindow via NSAppearanceCustomization
                // but is not yet in the generated objc2-app-kit 0.2 bindings.
                // Use msg_send! directly — the selector name matches Apple's SDK.
                let _: () = msg_send![&*ns_window, setAppearance: &*ns_appearance];
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}
