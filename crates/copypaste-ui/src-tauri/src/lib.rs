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

/// Handle to the "Private Mode" tray CheckMenuItem so the startup-race
/// re-sync handler (V-21-A) can update the checkmark after the daemon is
/// confirmed ready — without re-entering `setup_tray`.
///
/// `CheckMenuItem<Wry>` is internally `Arc`-backed, so cloning and storing
/// here is cheap.  The `Option` is `None` until `setup_tray` runs.
struct PrivateModeMenuItem(Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>);

// ---------------------------------------------------------------------------
// Tauri entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = UiConfig::default(); // will be overwritten in setup after handle is available
    tauri::Builder::default()
        .manage(CurrentShortcut(Mutex::new(cfg.popup_shortcut)))
        .manage(daemon_lifecycle::DaemonChild::default())
        .manage(daemon_lifecycle::DaemonSpawnError::default())
        .manage(daemon_lifecycle::DaemonLifecycleGen::default())
        // V-21-A: placeholder populated by setup_tray; background poller uses
        // it to re-sync the checkmark after the daemon socket becomes ready.
        .manage(PrivateModeMenuItem(Mutex::new(None)))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            ipc::ipc_call,
            ipc::pairing_qr_svg,
            ipc::reset_database,
            daemon_lifecycle::app_version,
            daemon_lifecycle::restart_daemon,
            daemon_lifecycle::get_daemon_error,
            get_popup_shortcut,
            set_popup_shortcut,
            check_accessibility_permission,
            request_accessibility_permission,
            start_recording_shortcut,
            stop_recording_shortcut,
            record_prior_app,
            paste_to_frontmost,
            hide_popup,
            play_copy_sound,
            show_copy_notification,
        ])
        .setup(|app| {
            // Load persisted config now that we have the app handle.
            let persisted = load_ui_config(app.handle());
            {
                let state: State<CurrentShortcut> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_shortcut.clone();
            }

            // App-owned daemon lifecycle: start the daemon on a background
            // thread so the tray and window render immediately (MED-b fix).
            // The result is stored in DaemonSpawnError state and emitted as
            // the "daemon-spawn-result" event; the UI reads it via
            // get_daemon_error or the event listener.
            daemon_lifecycle::ensure_daemon_running_async(app.handle().clone());

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

            // V-21-A: Startup race — the tray was built before the daemon socket
            // was necessarily ready, so `get_private_mode` may have defaulted to
            // false even though the daemon persisted private_mode=true.  Spawn a
            // background thread that polls until the daemon responds, then
            // re-syncs the CheckMenuItem to the daemon's true value.
            spawn_tray_private_mode_resync(app.handle().clone());

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
fn play_copy_sound() {
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
fn hide_popup_internal(handle: &tauri::AppHandle) {
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
            }
            let _ = handle.set_activation_policy(ActivationPolicy::Regular);
            return;
        }
    }

    if let Some(popup) = handle.get_webview_window("popup") {
        let _ = popup.hide();
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
fn hide_popup(handle: tauri::AppHandle) {
    hide_popup_internal(&handle);
}

/// Show a macOS notification banner after a successful copy.
///
/// Uses `osascript` to post a "display notification" so we don't need the
/// tauri-plugin-notification or any entitlement changes.  Any failure
/// (osascript missing, user denied Script Editor notifications, etc.) is
/// silently ignored — this is purely cosmetic feedback.
///
/// `preview` is a short one-line string supplied by the frontend (already
/// truncated to ≤60 chars).  The command sanitises it before embedding in the
/// AppleScript literal to prevent injection via quotes, backslashes, or
/// newlines/control chars (V-18 fix: newlines caused osascript to fail silently).
///
/// The command is cross-platform safe: on non-macOS it is a no-op.
#[tauri::command]
fn show_copy_notification(preview: String) {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Sanitise preview: replace quotes, backslashes, newlines, carriage
        // returns, and all other ASCII control characters with a space so they
        // cannot escape the AppleScript string literal or cause osascript to
        // fail silently on multi-line input (V-18 fix: \n was not stripped).
        let safe: String = preview
            .chars()
            .map(|c| {
                if c == '"' || c == '\\' || c == '\n' || c == '\r' || (c as u32) < 0x20 {
                    ' '
                } else {
                    c
                }
            })
            .take(60)
            .collect();
        let safe = safe.trim();

        let title = "CopyPaste";
        let body = if safe.is_empty() { "Copied" } else { safe };

        // Build the AppleScript.  Double-quote delimiters are already safe
        // because we stripped all `"` from the input above.
        let script = format!(r#"display notification "{body}" with title "{title}""#);

        // Spawn osascript on a background thread so we don't block the Tauri
        // command handler.  Errors are logged at DEBUG level; they never surface
        // to the user since this is purely cosmetic feedback.
        std::thread::spawn(move || {
            match Command::new("osascript").arg("-e").arg(&script).output() {
                Ok(out) if !out.status.success() => {
                    tracing::debug!(
                        "show_copy_notification: osascript exited {:?}: {}",
                        out.status.code(),
                        String::from_utf8_lossy(&out.stderr).trim()
                    );
                }
                Err(e) => {
                    tracing::debug!("show_copy_notification: failed to spawn osascript: {e}");
                }
                Ok(_) => {}
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = preview;
    }
}

/// Toggle (show or hide) the quick-paste popup near the current cursor position.
fn toggle_popup(handle: &tauri::AppHandle) {
    let Some(popup) = handle.get_webview_window("popup") else {
        tracing::warn!("popup window not found");
        return;
    };

    // If the popup is already visible, hide it via the shared internal helper
    // so the macOS prior-app activation runs (V-10 fix: was calling
    // popup.hide() directly, which skipped activation and surfaced main window).
    let is_visible = popup.is_visible().unwrap_or(false);
    if is_visible {
        hide_popup_internal(handle);
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

    // V-21-A: Store the CheckMenuItem in managed state so the background
    // daemon-ready poller (`spawn_tray_private_mode_resync`) can re-sync the
    // checkmark once the socket becomes available after a startup race.
    // CheckMenuItem<Wry> is Arc-backed; storing a clone here is cheap.
    {
        let state: tauri::State<PrivateModeMenuItem> = app.state();
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = Some(private_mode.clone());
    }

    // Second clone used by the on_menu_event closure below (V-21-B rollback).
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
                    // Tauri pre-toggles the checkmark before firing the event.
                    // Read the new (already-toggled) state from the cloned item.
                    let new_state = private_mode_clone.is_checked().unwrap_or(false);
                    let result = ipc::call(
                        "set_private_mode",
                        serde_json::json!({ "enabled": new_state }),
                    );
                    if let Err(e) = result {
                        // V-21-B: IPC failed — the daemon did not change state.
                        // Revert the checkmark so the tray reflects daemon truth
                        // rather than staying in the (incorrect) toggled position.
                        tracing::warn!("set_private_mode IPC error (reverting tray): {e}");
                        let _ = private_mode_clone.set_checked(!new_state);
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

/// V-21-A: Startup-race tray re-sync.
///
/// `setup_tray` runs synchronously during app setup, before the daemon socket
/// is necessarily bound.  If `get_private_mode` fails at that point the
/// checkmark defaults to false even though the daemon may have loaded
/// `private_mode = true` from its persisted settings.  This function spawns a
/// background thread that polls with short sleeps until the daemon responds,
/// then writes the real value back to the CheckMenuItem.
///
/// ## Stale-daemon race
///
/// `ensure_daemon_running_async` evicts any old daemon (SIGTERM + socket-
/// released poll) and spawns a fresh one, but this runs on a **separate**
/// background thread.  Because `setup_tray` and this resync thread both start
/// before eviction completes, the first successful IPC reply may come from the
/// **old** daemon (still alive during its graceful shutdown window).  If the
/// old daemon's in-memory state differs from the persisted file (e.g. a prior
/// `persist_private_mode` write failed silently), we would cache the stale
/// value and exit, leaving the tray desynchronised from the new daemon.
///
/// Guard: require **two consecutive, identical** successful IPC replies before
/// exiting.  The old daemon typically closes its socket within ~100 ms of
/// SIGTERM; the 250 ms poll interval gives it time to die.  If the first reply
/// came from the old daemon and the second call fails (socket gone), the
/// counter resets and we keep polling until the new daemon is stable.
fn spawn_tray_private_mode_resync(handle: tauri::AppHandle) {
    use std::thread;
    use std::time::{Duration, Instant};

    thread::spawn(move || {
        const POLL_INTERVAL: Duration = Duration::from_millis(250);
        const GIVE_UP_AFTER: Duration = Duration::from_secs(30);
        // Two consecutive identical replies are required before exiting.
        // This prevents caching a stale response from a dying old daemon.
        const CONFIRM_ROUNDS: usize = 2;

        let deadline = Instant::now() + GIVE_UP_AFTER;
        let mut last_value: Option<bool> = None;
        let mut confirm_count: usize = 0;

        loop {
            if Instant::now() >= deadline {
                tracing::warn!(
                    "tray private-mode re-sync: daemon not ready after 30 s — giving up"
                );
                return;
            }

            let result = ipc::call("get_private_mode", serde_json::json!({}));
            match result {
                Ok(reply) if reply.ok => {
                    let real_value = reply
                        .data
                        .as_ref()
                        .and_then(|d| d["private_mode"].as_bool())
                        .unwrap_or(false);

                    // Track consecutive identical responses to confirm stability.
                    if last_value == Some(real_value) {
                        confirm_count += 1;
                    } else {
                        last_value = Some(real_value);
                        confirm_count = 1;
                    }

                    // Update the CheckMenuItem immediately with the best-known value.
                    if let Some(state) = handle.try_state::<PrivateModeMenuItem>() {
                        if let Ok(guard) = state.0.lock() {
                            if let Some(ref item) = *guard {
                                // Only write if the value actually differs from
                                // what setup_tray already set, to avoid a
                                // spurious visual flicker.
                                let current = item.is_checked().unwrap_or(!real_value);
                                if current != real_value {
                                    tracing::info!(
                                        "tray private-mode re-sync: {} → {}",
                                        current,
                                        real_value
                                    );
                                    let _ = item.set_checked(real_value);
                                }
                            }
                        }
                    }

                    if confirm_count >= CONFIRM_ROUNDS {
                        // Stable for CONFIRM_ROUNDS consecutive polls — done.
                        return;
                    }
                    // Wait before the next confirmation poll.
                    thread::sleep(POLL_INTERVAL);
                }
                // Daemon not yet ready or socket changed; reset stability counter.
                _ => {
                    last_value = None;
                    confirm_count = 0;
                    thread::sleep(POLL_INTERVAL);
                }
            }
        }
    });
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
