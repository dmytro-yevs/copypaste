//! Tauri desktop UI for CopyPaste — a thin shell. The React frontend talks to
//! the daemon over the Unix-socket IPC via the `ipc_call` command (`ipc.rs`).
//! This crate never links `copypaste-core`; all data access is IPC-only.

mod daemon_lifecycle;
mod ipc;

#[cfg(target_os = "macos")]
mod event_tap;

use std::sync::Mutex;
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

const DEFAULT_POPUP_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
/// Config filename stored in the Tauri app-config directory.
const CONFIG_FILE: &str = "ui-config.json";

// ---------------------------------------------------------------------------
// Shortcut config — persisted to JSON
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct UiConfig {
    popup_shortcut: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            popup_shortcut: DEFAULT_POPUP_SHORTCUT.to_string(),
        }
    }
}

fn config_path(handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    handle
        .path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(CONFIG_FILE))
}

fn load_ui_config(handle: &tauri::AppHandle) -> UiConfig {
    let Some(path) = config_path(handle) else {
        return UiConfig::default();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_ui_config(handle: &tauri::AppHandle, cfg: &UiConfig) -> Result<(), String> {
    let path = config_path(handle).ok_or_else(|| "cannot determine app config dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("serialize config: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write config: {e}"))
}

// ---------------------------------------------------------------------------
// Managed state
// ---------------------------------------------------------------------------

struct CurrentShortcut(Mutex<String>);

/// Bundle ID (or process identifier as fallback) of the app that was
/// frontmost when the popup was last shown.  Used to restore focus after
/// the user picks an item.
#[cfg(target_os = "macos")]
struct PriorApp(Mutex<Option<String>>);

/// Whether the CGEventTap is active (Accessibility permission was granted and
/// the tap was successfully installed).
#[cfg(target_os = "macos")]
struct TapActive(Mutex<bool>);

// ---------------------------------------------------------------------------
// Tauri entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = UiConfig::default(); // will be overwritten in setup after handle is available
    tauri::Builder::default()
        .manage(CurrentShortcut(Mutex::new(cfg.popup_shortcut)))
        .manage(daemon_lifecycle::DaemonChild::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::ipc_call,
            ipc::pairing_qr_svg,
            get_popup_shortcut,
            set_popup_shortcut,
            check_accessibility_permission,
            request_accessibility_permission,
            start_recording_shortcut,
            stop_recording_shortcut,
            record_prior_app,
            paste_to_frontmost,
        ])
        .setup(|app| {
            // Load persisted config now that we have the app handle.
            let persisted = load_ui_config(app.handle());
            {
                let state: State<CurrentShortcut> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_shortcut.clone();
            }

            // App-owned daemon lifecycle: ensure the daemon is running BEFORE
            // the tray (which queries it) is built. Failures are surfaced loudly
            // via the log and the daemon-offline UI — never swallowed.
            if let Err(e) = daemon_lifecycle::ensure_daemon_running(app.handle()) {
                tracing::error!("failed to start app-owned daemon: {e}");
            }

            // Register macOS-only managed state.
            #[cfg(target_os = "macos")]
            {
                app.manage(PriorApp(Mutex::new(None)));
                app.manage(TapActive(Mutex::new(false)));
            }

            setup_tray(app)?;
            register_popup_shortcut(app.handle(), &persisted.popup_shortcut)?;
            setup_popup_window(app)?;
            setup_main_window(app);

            #[cfg(target_os = "macos")]
            {
                setup_macos(app);
                try_install_event_tap(app.handle(), &persisted.popup_shortcut);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building CopyPaste UI")
        .run(|handle, event| {
            // App-owned daemon lifecycle: when the WHOLE app exits, stop the
            // daemon we started. `RunEvent::Exit` fires only on a real quit
            // (tray "Quit" → `app.exit(0)`, or process termination). Closing
            // just the main WINDOW hides it to the tray (see
            // `setup_main_window`) and never reaches Exit, so the daemon
            // correctly survives a window close (standard macOS pattern).
            if let tauri::RunEvent::Exit = event {
                daemon_lifecycle::stop_daemon(handle);
            }
        });
}

// ---------------------------------------------------------------------------
// Tauri commands — shortcut management
// ---------------------------------------------------------------------------

/// Return the currently configured popup-shortcut accelerator string.
#[tauri::command]
fn get_popup_shortcut(state: State<'_, CurrentShortcut>) -> String {
    state.0.lock().expect("mutex poisoned").clone()
}

/// Change the popup shortcut at runtime and persist it.
///
/// Returns an error string if the accelerator string is invalid or already
/// registered by another application (e.g. an OS-reserved combo).
///
/// # OS-reserved shortcuts
/// This command uses the OS global-hotkey API via tauri-plugin-global-shortcut.
/// Combos reserved by the OS (e.g. Cmd+Space, Cmd+Tab on macOS) cannot be
/// intercepted by this API and will cause registration to fail with an error.
///
/// TODO: Intercepting OS-reserved keys would require installing a CGEventTap
/// (macOS Accessibility API, requires explicit user permission in
/// System Settings → Privacy & Security → Accessibility). That is a significant
/// UX and security trade-off and is not implemented here.
#[tauri::command]
fn set_popup_shortcut(
    accelerator: String,
    handle: tauri::AppHandle,
    state: State<'_, CurrentShortcut>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let old = {
        let guard = state.0.lock().expect("mutex poisoned");
        guard.clone()
    };

    // Unregister the old shortcut (best-effort; ignore errors if it wasn't registered).
    let _ = handle.global_shortcut().unregister(old.as_str());

    // Fix #2: attempt to register via the plugin.  On macOS, OS-reserved or
    // shift+alt combos (e.g. Alt+Shift+Q which produces "Œ") may fail here.
    // When the CGEventTap is active it will handle the shortcut anyway, so we
    // treat the plugin registration error as a non-fatal warning in that case.
    let plugin_result = register_popup_shortcut(&handle, &accelerator);
    #[cfg(target_os = "macos")]
    {
        let tap_active = handle
            .try_state::<TapActive>()
            .map(|s| *s.0.lock().expect("mutex poisoned"))
            .unwrap_or(false);
        if !tap_active {
            // No tap — plugin must succeed.
            plugin_result.map_err(|e| e.to_string())?;
        } else {
            // Tap is running; log plugin failure but don't surface it as an error.
            if let Err(e) = plugin_result {
                tracing::warn!(
                    "plugin global-shortcut registration for '{accelerator}' failed \
                     (CGEventTap will handle it): {e}"
                );
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    plugin_result.map_err(|e| e.to_string())?;

    // Persist the new accelerator.
    let new_cfg = UiConfig {
        popup_shortcut: accelerator.clone(),
    };
    save_ui_config(&handle, &new_cfg)?;

    // Update in-memory state.
    {
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = accelerator.clone();
    }

    // Keep the CGEventTap in sync (macOS only, no-op if tap is not running).
    #[cfg(target_os = "macos")]
    event_tap::update_tap_shortcut(&accelerator);

    Ok(())
}

// ---------------------------------------------------------------------------
// Accessibility + CGEventTap commands (macOS)
// ---------------------------------------------------------------------------

/// Returns `true` when the Accessibility permission is granted.
/// On non-macOS platforms this always returns `true` (no permission needed).
#[tauri::command]
fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        event_tap::accessibility_granted()
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
fn request_accessibility_permission(handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        event_tap::open_accessibility_settings();
        // The user may already have granted permission before clicking;
        // try installing the tap in the background.
        let shortcut = {
            let state: State<CurrentShortcut> = handle.state();
            let s = state.0.lock().expect("mutex poisoned").clone();
            s
        };
        try_install_event_tap(&handle, &shortcut);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// Begin a one-shot HID-level shortcut recording session.
///
/// Installs a CGEventTap at `kCGHIDEventTapLocation` (below Hammerspoon's
/// session tap) that captures the next key chord and emits a `shortcut-recorded`
/// event on `window` with payload `{ accelerator: String }`.
///
/// Returns an error if Accessibility permission is not granted or the tap
/// could not be installed.
///
/// NOTE: Input Monitoring permission is also required on macOS 10.15+.  If the
/// tap fails to install, the user should grant both Accessibility **and**
/// Input Monitoring in System Settings → Privacy & Security.
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn start_recording_shortcut(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        event_tap::start_recording(move |accel| {
            // Deliver the captured accelerator to the frontend via a Tauri event.
            let _ = window.emit(
                "shortcut-recorded",
                serde_json::json!({ "accelerator": accel }),
            );
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("shortcut recording is only supported on macOS".into())
    }
}

/// Cancel an in-progress shortcut recording session (no-op if not recording).
#[tauri::command]
fn stop_recording_shortcut() {
    #[cfg(target_os = "macos")]
    event_tap::stop_recording();
}

/// Save the bundle ID of the currently frontmost application so we can
/// restore focus after the popup closes.  Call this just before showing the
/// popup (from the JS layer via `toggle_popup` or directly before `win.show`).
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn record_prior_app(handle: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let bundle_id = frontmost_bundle_id();
        if let Some(state) = handle.try_state::<PriorApp>() {
            let mut guard = state.0.lock().expect("mutex poisoned");
            *guard = bundle_id;
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}

/// Activate the previously-focused application (restoring focus) and then
/// synthesise a Cmd+V paste event so the clipboard content lands in the
/// target app.  Call this after `api.copyItem` and before hiding the popup.
#[tauri::command]
fn paste_to_frontmost(handle: tauri::AppHandle) -> Result<(), String> {
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

        // Activate the prior app, then after a short delay synthesise Cmd+V.
        thread::spawn(move || {
            // Activate the prior app by bundle ID (best effort).
            if let Some(ref bid) = bundle_id {
                activate_app_by_bundle_id(bid);
            }

            // Give the app a moment to come to the foreground.
            thread::sleep(Duration::from_millis(80));

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

// ---------------------------------------------------------------------------
// macOS helpers
// ---------------------------------------------------------------------------

/// Try to install the CGEventTap.  Silently logs and falls back to the
/// plugin-based shortcut if Accessibility is not granted.
#[cfg(target_os = "macos")]
fn try_install_event_tap(handle: &tauri::AppHandle, accel: &str) {
    let tap_state = handle.try_state::<TapActive>();
    // Already installed?
    if let Some(ref ts) = tap_state {
        if *ts.0.lock().expect("mutex poisoned") {
            // Just update the accelerator.
            event_tap::update_tap_shortcut(accel);
            return;
        }
    }

    let handle_clone = handle.clone();
    match event_tap::install(accel, move || {
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

/// Return the bundle identifier of the currently frontmost application,
/// or `None` if it cannot be determined.
#[cfg(target_os = "macos")]
fn frontmost_bundle_id() -> Option<String> {
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

/// Activate the app with the given bundle identifier using NSRunningApplication.
#[cfg(target_os = "macos")]
fn activate_app_by_bundle_id(bundle_id: &str) {
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Register the global shortcut that toggles the quick-paste popup.
fn register_popup_shortcut(
    handle: &tauri::AppHandle,
    accelerator: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    let handle_clone = handle.clone();
    handle
        .global_shortcut()
        .on_shortcut(accelerator, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_popup(&handle_clone);
            }
        })
        .map_err(|e| -> Box<dyn std::error::Error> {
            format!("global_shortcut register error: {e}").into()
        })?;

    Ok(())
}

/// Toggle (show or hide) the quick-paste popup near the current cursor position.
fn toggle_popup(handle: &tauri::AppHandle) {
    let Some(popup) = handle.get_webview_window("popup") else {
        tracing::warn!("popup window not found");
        return;
    };

    // If the popup is already visible, hide it.
    let is_visible = popup.is_visible().unwrap_or(false);
    if is_visible {
        let _ = popup.hide();
        return;
    }

    position_popup_near_cursor(&popup);

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

/// Position the popup window near the current cursor, clamped to the monitor
/// that contains the cursor.
///
/// Fix #4: iterate all available monitors and pick the one whose physical-pixel
/// bounds contain the cursor (handles negative coordinates on secondary monitors
/// and avoids `monitor_from_point` misses on multi-display setups).
///
/// All arithmetic is done in physical pixels:
///   - `cursor_position()` returns physical px
///   - `monitor.position()` / `monitor.size()` are already in physical px
///   - `monitor.scale_factor()` converts logical popup dims to physical px
///   - `set_position(PhysicalPosition)` places the window in physical px
fn position_popup_near_cursor(win: &tauri::WebviewWindow) {
    // cursor_position() → physical pixels.
    let cursor_pos: tauri::PhysicalPosition<i32> = win
        .cursor_position()
        .map(|p| tauri::PhysicalPosition {
            x: p.x as i32,
            y: p.y as i32,
        })
        .unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });

    // Logical popup dimensions from tauri.conf.json.
    const POPUP_W_LOGICAL: f64 = 420.0;
    const POPUP_H_LOGICAL: f64 = 520.0;
    // Small offset so the popup doesn't sit right on the cursor tip.
    const OFFSET: i32 = 8;

    let mut x = cursor_pos.x + OFFSET;
    let mut y = cursor_pos.y + OFFSET;

    // Fix #4: find the monitor whose physical bounds contain the cursor by
    // iterating all monitors instead of relying on monitor_from_point, which
    // can pick the wrong display on some multi-monitor configurations.
    let monitors = win.available_monitors().unwrap_or_default();

    // Helper: check if cursor falls within a monitor's physical rect.
    let find_monitor = |cx: i32, cy: i32| -> Option<tauri::Monitor> {
        monitors
            .iter()
            .find(|m| {
                let pos = m.position();
                let size = m.size();
                let mx = pos.x;
                let my = pos.y;
                let mw = size.width as i32;
                let mh = size.height as i32;
                cx >= mx && cx < mx + mw && cy >= my && cy < my + mh
            })
            .cloned()
    };

    // Try the cursor position first; fall back to primary monitor.
    let monitor_opt =
        find_monitor(cursor_pos.x, cursor_pos.y).or_else(|| win.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor_opt {
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

        x = x.clamp(mon_x, max_x.max(mon_x));
        y = y.clamp(mon_y, max_y.max(mon_y));
    }

    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Wire the main window so closing it HIDES it to the tray instead of quitting
/// the whole app. This is the standard macOS menu-bar pattern and is what keeps
/// the app-owned daemon alive on a window close: only a real Quit (tray "Quit"
/// → `app.exit(0)`) terminates the process and triggers `stop_daemon`.
fn setup_main_window(app: &tauri::App) {
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

/// Ensure the popup window is created (it may already be created via tauri.conf.json).
/// Attach a blur/focus-loss handler that auto-hides the popup.
fn setup_popup_window(app: &tauri::App) -> tauri::Result<()> {
    // The popup window is declared in tauri.conf.json; just look it up or
    // create it lazily if needed (in test environments the conf may omit it).
    let popup = if let Some(w) = app.get_webview_window("popup") {
        w
    } else {
        WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("popup.html".into()))
            .title("CopyPaste Quick Paste")
            .inner_size(420.0, 520.0)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()?
    };

    // Hide on focus loss (user clicked away).
    popup.on_window_event(|event| {
        if let tauri::WindowEvent::Focused(false) = event {
            // The window itself is `self` inside this closure; we cannot call
            // .hide() here because we only have &WindowEvent, not the handle.
            // We emit a JS event instead; the frontend listens and calls
            // getCurrentWindow().hide() on Esc / blur.  The native hide on blur
            // is handled by the JS focus-out listener in Popup.tsx.
        }
    });

    // Apply vibrancy on macOS.
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        let _ = apply_vibrancy(
            &popup,
            NSVisualEffectMaterial::HudWindow,
            Some(NSVisualEffectState::Active),
            Some(12.0),
        );
    }

    Ok(())
}

/// Truncate a preview string to at most `max_chars` characters, collapsing
/// interior newlines to a single space and appending "…" when cut.
fn truncate_preview(s: &str, max_chars: usize) -> String {
    // Collapse newlines / tabs into a space so the label is single-line.
    let flat: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let flat = flat.trim();
    let chars: Vec<char> = flat.chars().collect();
    if chars.len() <= max_chars {
        chars.iter().collect()
    } else {
        // Leave room for the ellipsis character.
        let cut: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{}…", cut.trim_end())
    }
}

/// Build and register the menu-bar tray icon.
///
/// Gracefully degrades when the daemon is offline: Recent submenu shows a
/// disabled "No recent items" entry, and Private Mode defaults to unchecked.
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use serde_json::json;
    use tauri::menu::{
        CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
    };
    use tauri::tray::TrayIconBuilder;

    // --- "Open CopyPaste" ---
    let open = MenuItemBuilder::with_id("open", "Open CopyPaste").build(app)?;

    // --- "Recent" submenu ---
    // Fetch up to 10 recent items from the daemon. If the daemon is offline or
    // returns an empty list we show a single disabled placeholder entry.
    let recent_submenu = {
        let mut builder = SubmenuBuilder::new(app, "Recent");

        let items_opt: Option<Vec<(String, String)>> =
            ipc::call("history_page", json!({ "limit": 10, "offset": 0 }))
                .ok()
                .and_then(|reply| {
                    if !reply.ok {
                        return None;
                    }
                    reply
                        .data
                        .as_ref()
                        .and_then(|d| d["items"].as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|item| {
                                    let id = item["id"].as_str()?.to_owned();
                                    let preview = item["preview"].as_str().unwrap_or("").to_owned();
                                    Some((id, preview))
                                })
                                .collect::<Vec<_>>()
                        })
                });

        match items_opt {
            Some(items) if !items.is_empty() => {
                for (id, preview) in &items {
                    let label = truncate_preview(preview, 40);
                    let menu_id = format!("recent:{id}");
                    let item = MenuItemBuilder::with_id(menu_id, label).build(app)?;
                    builder = builder.item(&item);
                }
            }
            _ => {
                // Daemon offline or no items — show a disabled placeholder.
                let placeholder = MenuItemBuilder::with_id("recent:none", "No recent items")
                    .enabled(false)
                    .build(app)?;
                builder = builder.item(&placeholder);
            }
        }

        builder.build()?
    };

    // --- "Private Mode" check item ---
    // Query the daemon for the current state; fall back to false on any error.
    let private_mode_on: bool = ipc::call("get_private_mode", json!({}))
        .ok()
        .and_then(|reply| {
            if !reply.ok {
                return None;
            }
            reply
                .data
                .as_ref()
                .and_then(|d| d["private_mode"].as_bool())
        })
        .unwrap_or(false);

    let private_mode = CheckMenuItemBuilder::with_id("private_mode", "Private Mode")
        .checked(private_mode_on)
        .build(app)?;

    // Clone the CheckMenuItem so the event closure can read its checked state.
    // CheckMenuItem<R> is internally Arc-backed, so clone is cheap.
    let private_mode_clone = private_mode.clone();

    // --- Separator + Quit ---
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit CopyPaste").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&open, &recent_submenu])
        .item(&private_mode)
        .item(&separator)
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "open" => show_main(app),
                "quit" => app.exit(0),
                "private_mode" => {
                    // Read the new checked state directly from the cloned item.
                    // Tauri has already toggled the check mark before firing the event.
                    let new_state = private_mode_clone.is_checked().unwrap_or(false);
                    let result = ipc::call(
                        "set_private_mode",
                        serde_json::json!({ "enabled": new_state }),
                    );
                    if let Err(e) = result {
                        tracing::warn!("set_private_mode IPC error: {e}");
                    }
                }
                other if other.starts_with("recent:") && other != "recent:none" => {
                    let item_id = &other["recent:".len()..];
                    let result = ipc::call("copy_item", serde_json::json!({ "id": item_id }));
                    if let Err(e) = result {
                        tracing::warn!("copy_item IPC error: {e}");
                    }
                }
                _ => {}
            }
        });

    let tray_img =
        tauri::image::Image::from_bytes(include_bytes!("../../assets/tray-icon-32.png"))?;
    builder = builder.icon(tray_img).icon_as_template(true);
    builder.build(app)?;
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn setup_macos(app: &tauri::App) {
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
    }
}
