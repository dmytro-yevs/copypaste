//! Tauri desktop UI for CopyPaste — a thin shell. The React frontend talks to
//! the daemon over the Unix-socket IPC via the `ipc_call` command (`ipc.rs`).
//! This crate never links `copypaste-core`; all data access is IPC-only.

mod daemon_lifecycle;
mod ipc;

#[cfg(target_os = "macos")]
mod event_tap;

use std::sync::Mutex;
use tauri::{Emitter, Listener, Manager, State, WebviewUrl, WebviewWindowBuilder};

const DEFAULT_POPUP_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
/// Config filename stored in the Tauri app-config directory.
const CONFIG_FILE: &str = "ui-config.json";

// ---------------------------------------------------------------------------
// Shortcut + launch + position config — persisted to JSON
// ---------------------------------------------------------------------------

/// Popup position mode.  Variants must match the string values the React
/// layer sends/receives so they are serialised as lowercase strings.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
enum PopupPosition {
    /// Show near the mouse cursor (Maccy default).
    #[default]
    Cursor,
    /// Center of the active screen.
    Center,
    /// Below the tray / menu-bar icon (top-right of the primary display).
    Menubar,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct UiConfig {
    /// Per-field default so a config written by a different version (or one
    /// missing this key) still loads instead of silently resetting every
    /// setting back to defaults.
    #[serde(default = "default_popup_shortcut")]
    popup_shortcut: String,
    /// Auto-start CopyPaste at macOS login.  Defaults to `true` so a fresh
    /// install is convenient out-of-the-box; can be disabled in Settings.
    #[serde(default = "default_launch_at_login")]
    launch_at_login: bool,
    /// Where to position the quick-paste popup when shown.
    #[serde(default)]
    popup_position: PopupPosition,
}

fn default_launch_at_login() -> bool {
    true
}

fn default_popup_shortcut() -> String {
    DEFAULT_POPUP_SHORTCUT.to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            popup_shortcut: DEFAULT_POPUP_SHORTCUT.to_string(),
            launch_at_login: true,
            popup_position: PopupPosition::default(),
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
    let Ok(contents) = std::fs::read_to_string(&path) else {
        // Missing file on first run is normal; fall back to defaults silently.
        return UiConfig::default();
    };
    match serde_json::from_str(&contents) {
        Ok(cfg) => cfg,
        Err(e) => {
            // Don't silently wipe everything: at least surface the reason so a
            // bad/older config that resets the saved shortcut is diagnosable.
            tracing::warn!(
                "ui-config.json failed to parse ({e}); using defaults — \
                 saved popup shortcut / position may be reset"
            );
            UiConfig::default()
        }
    }
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

/// Current popup-position mode.  Wrapped in Mutex so commands can update it
/// without reloading the full config from disk on every call.
struct CurrentPopupPosition(Mutex<PopupPosition>);

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

/// Handle to the "Recent" tray Submenu so the background poller can
/// rebuild it once the daemon is ready and periodically thereafter.
///
/// `Submenu<Wry>` is internally `Arc`-backed; cloning is cheap.
/// The `Option` is `None` until `setup_tray` runs.
struct RecentSubmenu(Mutex<Option<tauri::menu::Submenu<tauri::Wry>>>);

/// Stop flag for `spawn_tray_recent_resync`. Set to `true` in `RunEvent::Exit`
/// so the background polling loop exits cleanly instead of holding the
/// `AppHandle` forever and blocking teardown.
struct TrayResyncStop(std::sync::Arc<std::sync::atomic::AtomicBool>);

// ---------------------------------------------------------------------------
// Tauri entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = UiConfig::default(); // will be overwritten in setup after handle is available
    tauri::Builder::default()
        .manage(CurrentShortcut(Mutex::new(cfg.popup_shortcut)))
        .manage(CurrentPopupPosition(Mutex::new(cfg.popup_position)))
        .manage(daemon_lifecycle::DaemonChild::default())
        .manage(daemon_lifecycle::DaemonSpawnError::default())
        .manage(daemon_lifecycle::DaemonLifecycleGen::default())
        // V-21-A: placeholder populated by setup_tray; background poller uses
        // it to re-sync the checkmark after the daemon socket becomes ready.
        .manage(PrivateModeMenuItem(Mutex::new(None)))
        // Recent-resync: placeholder populated by setup_tray; background poller
        // rebuilds the submenu once the daemon responds and then periodically.
        .manage(RecentSubmenu(Mutex::new(None)))
        // Stop flag for the recent-resync background thread; set on app exit.
        .manage(TrayResyncStop(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(false),
        )))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            // No extra CLI args needed; the app launches the daemon itself.
            None::<Vec<&str>>,
        ))
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
            get_launch_at_login,
            set_launch_at_login,
            get_popup_position,
            set_popup_position,
            ipc::read_logs,
            ipc::log_dir_path,
            ipc::ingest_dropped_files,
            focus_main_window,
        ])
        .setup(|app| {
            // Load persisted config now that we have the app handle.
            let persisted = load_ui_config(app.handle());
            {
                let state: State<CurrentShortcut> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_shortcut.clone();
            }
            {
                let state: State<CurrentPopupPosition> = app.state();
                let mut guard = state.0.lock().expect("mutex poisoned");
                *guard = persisted.popup_position.clone();
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

            // Apply persisted launch-at-login preference idempotently.
            apply_launch_at_login(app.handle(), persisted.launch_at_login);

            setup_tray(app)?;
            // Non-fatal: a saved accelerator the OS/plugin can't register
            // (reserved combo, or a format the plugin rejects) must NOT crash
            // app startup. The value is already loaded into CurrentShortcut
            // above, and on macOS the CGEventTap can still handle it.
            if let Err(e) = register_popup_shortcut(app.handle(), &persisted.popup_shortcut) {
                tracing::warn!(
                    "startup: failed to register popup shortcut '{}': {e} \
                     (value preserved; re-set it in Settings if it doesn't trigger)",
                    persisted.popup_shortcut
                );
            }
            // M1: popup is lazy-created on first hotkey press (toggle_popup),
            // not at app launch — saves ~84 MB idle RSS.
            setup_main_window(app);

            // V-21-A: Startup race — the tray was built before the daemon socket
            // was necessarily ready, so `get_private_mode` may have defaulted to
            // false even though the daemon persisted private_mode=true.  Spawn a
            // background thread that polls until the daemon responds, then
            // re-syncs the CheckMenuItem to the daemon's true value.
            spawn_tray_private_mode_resync(app.handle().clone());

            // M4: Push path — refresh the tray CheckMenuItem whenever private
            // mode is toggled anywhere (Settings window, or the tray itself
            // re-emitting after its own toggle). The Settings frontend emits
            // `private-mode-changed` with the daemon-confirmed bool as payload.
            {
                let listen_handle = app.handle().clone();
                app.listen("private-mode-changed", move |event| {
                    // Payload is a JSON-encoded bool (e.g. "true").
                    let Ok(new_value) = serde_json::from_str::<bool>(event.payload()) else {
                        tracing::warn!(
                            "private-mode-changed: unparseable payload {:?}",
                            event.payload()
                        );
                        return;
                    };
                    // Reuse the exact CheckMenuItem update pattern from the
                    // startup re-sync handler (spawn_tray_private_mode_resync).
                    if let Some(state) = listen_handle.try_state::<PrivateModeMenuItem>() {
                        if let Ok(guard) = state.0.lock() {
                            if let Some(ref item) = *guard {
                                let current = item.is_checked().unwrap_or(!new_value);
                                if current != new_value {
                                    let _ = item.set_checked(new_value);
                                }
                            }
                        }
                    }
                });
            }

            // Recent-resync: the tray Recent submenu was built at startup before
            // the daemon was ready, so it likely shows a placeholder.  Spawn a
            // background poller that rebuilds it once the daemon responds and
            // then refreshes it periodically so the items stay current.
            spawn_tray_recent_resync(app.handle().clone());

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
                // Signal the recent-resync background thread to exit before we
                // tear down the daemon, so it doesn't make IPC calls against a
                // dead socket during teardown.
                if let Some(stop) = handle.try_state::<TrayResyncStop>() {
                    stop.0.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                // Release the macOS CGEventTap (tap, run-loop source, CFMachPort,
                // and the boxed trigger callback) so nothing leaks on quit.
                #[cfg(target_os = "macos")]
                event_tap::uninstall();
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

    // Unregister the old shortcut (best-effort). Log a warning when this fails
    // so a ghost shortcut registration doesn't go unnoticed.
    if let Err(e) = handle.global_shortcut().unregister(old.as_str()) {
        tracing::warn!(
            "set_popup_shortcut: failed to unregister old shortcut '{}': {e} \
             (ghost registration possible — old hotkey may still be active)",
            old
        );
    }

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

    // Persist the new accelerator (preserving other config fields).
    let mut new_cfg = load_ui_config(&handle);
    new_cfg.popup_shortcut = accelerator.clone();
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
// Launch-at-login commands
// ---------------------------------------------------------------------------

/// Returns `true` when the app is registered to launch at macOS login.
#[tauri::command]
fn get_launch_at_login(handle: tauri::AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    handle.autolaunch().is_enabled().unwrap_or(false)
}

/// Enable or disable launching CopyPaste at macOS login.
///
/// The setting is persisted to `ui-config.json` so it survives reinstalls and
/// re-reads on the next launch via `apply_launch_at_login`.
#[tauri::command]
fn set_launch_at_login(enabled: bool, handle: tauri::AppHandle) -> Result<(), String> {
    apply_launch_at_login(&handle, enabled);
    // Persist to config.
    let mut cfg = load_ui_config(&handle);
    cfg.launch_at_login = enabled;
    save_ui_config(&handle, &cfg)
}

/// Idempotently sync the OS launch-agent state with the desired value.
///
/// Called both at startup (to enforce the persisted preference) and by
/// `set_launch_at_login`.  Errors are logged but not surfaced — a failure to
/// register/deregister a LaunchAgent is non-fatal for the running app.
fn apply_launch_at_login(handle: &tauri::AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = handle.autolaunch();
    let current = mgr.is_enabled().unwrap_or(false);
    if enabled && !current {
        if let Err(e) = mgr.enable() {
            tracing::warn!("launch-at-login enable failed: {e}");
        }
    } else if !enabled && current {
        if let Err(e) = mgr.disable() {
            tracing::warn!("launch-at-login disable failed: {e}");
        }
    }
    // If current == desired, nothing to do (idempotent).
}

// ---------------------------------------------------------------------------
// Popup-position commands
// ---------------------------------------------------------------------------

/// Return the current popup-position mode as a string ("cursor", "center", "menubar").
#[tauri::command]
fn get_popup_position(state: State<'_, CurrentPopupPosition>) -> String {
    let guard = state.0.lock().expect("mutex poisoned");
    // serde_json serialises the enum as a lowercase string; strip the quotes.
    serde_json::to_value(&*guard)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "cursor".to_owned())
}

/// Set the popup-position mode.  Accepted values: "cursor", "center", "menubar".
/// Returns an error string for unknown values.
#[tauri::command]
fn set_popup_position(
    mode: String,
    handle: tauri::AppHandle,
    state: State<'_, CurrentPopupPosition>,
) -> Result<(), String> {
    let pos: PopupPosition = serde_json::from_value(serde_json::Value::String(mode.clone()))
        .map_err(|_| {
            format!("unknown popup position mode: {mode:?} (expected cursor|center|menubar)")
        })?;
    {
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = pos.clone();
    }
    let mut cfg = load_ui_config(&handle);
    cfg.popup_position = pos;
    save_ui_config(&handle, &cfg)
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
fn hide_popup(handle: tauri::AppHandle) {
    hide_popup_internal(&handle);
}

/// Show a rich macOS notification banner after a successful copy.
///
/// Posts via `UNUserNotificationCenter` from inside the `CopyPaste.app`
/// bundle so the notification automatically shows the app icon.  This
/// replaces the old `osascript display notification` path which ran from
/// a process with no bundle identity and therefore showed a generic
/// `Script Editor` icon with no app icon and no rich preview.
///
/// Parameters (set by the frontend):
/// - `title`: short type label, e.g. "Text Copied", "Image Copied",
///   "File Copied".
/// - `body`: item preview — first ~160 chars of text (newlines preserved,
///   truncated with `…`), the filename for files, or "Image" for images.
///
/// Authorization: on macOS 10.14+ the first call triggers the system
/// permission prompt.  If the user denies it or the request fails, the
/// error is silently swallowed — this is purely cosmetic feedback.
///
/// The command is cross-platform safe: on non-macOS it is a no-op.
#[tauri::command]
fn show_copy_notification(title: String, body: String) {
    #[cfg(target_os = "macos")]
    {
        post_un_notification(title, body);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body);
    }
}

/// Post a `UNUserNotificationCenter` banner from the app bundle.
///
/// Called on a background thread (spawned by the Tauri command handler or
/// the background-capture poller) so the main run-loop is never blocked.
/// Any failure is logged at DEBUG level and silently swallowed.
#[cfg(target_os = "macos")]
fn post_un_notification(title: String, body: String) {
    std::thread::spawn(move || {
        use block2::RcBlock;
        // objc2_foundation_v3 is the aliased objc2-foundation 0.3.x crate that
        // matches the types used by objc2-user-notifications 0.3.x (which
        // depends on objc2 0.6.x).  The rest of the crate uses the 0.2.x
        // bindings required by objc2-app-kit 0.2.x; both coexist in the graph.
        use objc2_foundation_v3::{NSError, NSString};
        // Bool from objc2 0.6.x — must match the version used by
        // objc2-user-notifications 0.3.x for the IntoBlock impl to unify.
        use objc2_user_notifications::{
            UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
            UNUserNotificationCenter,
        };
        use objc2_v6::runtime::Bool;

        // SAFETY: All ObjC calls below are on the same thread; objc2 0.6.x
        // enforces Send/Sync on retained objects so cross-thread use is safe.
        // `UNUserNotificationCenter::currentNotificationCenter()` is documented
        // as safe to call from any thread.
        unsafe {
            let center = UNUserNotificationCenter::currentNotificationCenter();

            // Request `.alert` authorization — shows a system prompt on first
            // call; subsequent calls return the cached decision immediately.
            let auth_opts = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Badge;
            // The closure parameter types must match the DynBlock signature
            // generated by objc2-user-notifications 0.3.x:
            //   dyn Fn(Bool, *mut NSError)
            // where Bool and NSError come from objc2 0.6.x / objc2-foundation 0.3.x.
            let auth_block = RcBlock::new(move |granted: Bool, err: *mut NSError| {
                if !granted.as_bool() {
                    if err.is_null() {
                        tracing::debug!("post_un_notification: notification permission denied");
                    } else {
                        // SAFETY: err is non-null and owned by the ObjC runtime.
                        let msg = (*err).localizedDescription();
                        tracing::debug!("post_un_notification: auth error: {}", msg);
                    }
                }
            });
            center.requestAuthorizationWithOptions_completionHandler(auth_opts, &auth_block);

            // Build content — title + body.
            // NSString::from_str returns Retained<NSString>; bind before
            // passing so we can deref to &NSString as required by setTitle/setBody.
            let content = UNMutableNotificationContent::new();
            let ns_title = NSString::from_str(&title);
            let ns_body = NSString::from_str(&body);
            content.setTitle(&ns_title);
            content.setBody(&ns_body);

            // Unique identifier per notification so each fires independently.
            let req_id = NSString::from_str(&format!(
                "com.copypaste.copy.{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));

            // trigger = None → deliver immediately.
            // req_id is Retained<NSString>; deref to &NSString for the call.
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &req_id, &content, None,
            );

            // Completion block: dyn Fn(*mut NSError) — same NSError from 0.3.x.
            let done_block = RcBlock::new(move |err: *mut NSError| {
                if !err.is_null() {
                    // SAFETY: err is non-null and owned by the ObjC runtime.
                    let msg = (*err).localizedDescription();
                    tracing::debug!("post_un_notification: add request failed: {}", msg);
                }
            });
            center.addNotificationRequest_withCompletionHandler(&request, Some(&done_block));
        }
    });
}

/// Toggle (show or hide) the quick-paste popup using the configured position mode.
///
/// M1: Lazy-create the popup WebView on the first toggle instead of at app
/// launch — saves ~84 MB of idle RSS (full WKWebView process + JS heap that
/// was previously sitting warm even when the popup was never opened).
/// The warm path (window already created) is unaffected: only the JS heap is
/// freed on hide (via `window.__copypasteFreeMemory`), not the WebView itself,
/// so show-latency stays instant.
fn toggle_popup(handle: &tauri::AppHandle) {
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

/// Logical popup dimensions (must match tauri.conf.json).
const POPUP_W_LOGICAL: f64 = 403.0; // v0.5.3: matches tauri.conf.json popup width (was 504)
const POPUP_H_LOGICAL: f64 = 624.0; // v0.5.3: 1.2× enlargement (was 520)

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
    // The submenu handle is stored in RecentSubmenu managed state so the
    // background poller can rebuild it once the daemon is ready.
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

    // Store a clone of the submenu handle in managed state so the background
    // poller can rebuild it without re-entering setup_tray.
    {
        let state: tauri::State<RecentSubmenu> = app.state();
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = Some(recent_submenu.clone());
    }

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
                    match result {
                        Ok(_) => {
                            // M4: Broadcast the confirmed toggle so the Settings
                            // window (and any other listener) converges on the
                            // same value, regardless of where the toggle began.
                            let _ = app.emit("private-mode-changed", new_state);
                        }
                        Err(e) => {
                            // V-21-B: IPC failed — the daemon did not change state.
                            // Revert the checkmark so the tray reflects daemon truth
                            // rather than staying in the (incorrect) toggled position.
                            tracing::warn!("set_private_mode IPC error (reverting tray): {e}");
                            let _ = private_mode_clone.set_checked(!new_state);
                            // Broadcast the reverted (daemon-truth) value too.
                            let _ = app.emit("private-mode-changed", !new_state);
                        }
                    }
                }
                other if other.starts_with("recent:") && other != "recent:none" => {
                    let item_id = &other["recent:".len()..];
                    let result = ipc::call("copy_item", serde_json::json!({ "id": item_id }));
                    match &result {
                        Ok(reply) if reply.ok => {
                            // Mirror the sound/notification that row-click copy fires so
                            // tray copies are consistent with the "always sound on copy"
                            // promise (audit finding P1 / M12 parity).
                            play_copy_sound();
                            // Build rich title + body from the content_type / preview
                            // returned by the copy_item IPC response.
                            let (title, body) = notification_title_body_from_reply(reply);
                            show_copy_notification(title, body);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!("copy_item IPC error: {e}");
                        }
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

/// Bring the main CopyPaste window to the foreground.
///
/// Called by the React layer when an incoming pairing request (role=responder,
/// state=awaiting_sas) is detected so the SAS confirmation modal is visible
/// to the user immediately, without requiring them to open the app manually.
#[tauri::command]
fn focus_main_window(handle: tauri::AppHandle) {
    show_main(&handle);
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

/// Rebuild the Recent tray submenu from a fresh history_page call.
///
/// Clears all existing items in the submenu and repopulates with up to 10
/// items from the daemon. Falls back to the "No recent items" placeholder if
/// the daemon is offline or returns an empty list. The submenu handle is
/// shared via `RecentSubmenu` managed state; no re-registration of the tray
/// menu is needed because `Submenu<Wry>` is Arc-backed and mutations are
/// reflected live in the displayed menu.
///
/// The existing `on_menu_event` handler dispatches on `other.starts_with("recent:")`
/// so it automatically handles any item ID written here — no re-registration needed.
fn rebuild_recent_submenu(
    handle: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tauri::menu::MenuItemBuilder;

    let state = handle
        .try_state::<RecentSubmenu>()
        .ok_or("RecentSubmenu state not registered")?;
    let guard = state.0.lock().map_err(|e| format!("mutex poisoned: {e}"))?;
    let submenu = guard
        .as_ref()
        .ok_or("RecentSubmenu not yet populated by setup_tray")?;

    // Fetch up to 10 items. On any error, fall back to a placeholder.
    let items_opt: Option<Vec<(String, String)>> = ipc::call(
        "history_page",
        serde_json::json!({ "limit": 10, "offset": 0 }),
    )
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

    // Remove all existing items (iterate in reverse so indices stay valid).
    let existing = submenu.items()?;
    for i in (0..existing.len()).rev() {
        let _ = submenu.remove_at(i);
    }

    // Append fresh items.
    match items_opt {
        Some(items) if !items.is_empty() => {
            for (id, preview) in &items {
                let label = truncate_preview(preview, 40);
                let menu_id = format!("recent:{id}");
                let item = MenuItemBuilder::with_id(menu_id, label).build(handle)?;
                submenu.append(&item)?;
            }
        }
        _ => {
            let placeholder = MenuItemBuilder::with_id("recent:none", "No recent items")
                .enabled(false)
                .build(handle)?;
            submenu.append(&placeholder)?;
        }
    }

    Ok(())
}

/// Build a rich notification `(title, body)` pair from a `copy_item` IPC
/// reply.
///
/// The daemon now returns `content_type` and `preview` in the `copy_item`
/// response.  This helper maps them to the human-readable strings shown in
/// the macOS notification banner.
///
/// - Text → `("Text Copied", first ~160 chars truncated with …)`
/// - Image → `("Image Copied", "Image")`
/// - File → `("File Copied", filename extracted from "[file: <name>]")`
/// - Unknown → `("Copied", preview-as-body or "Copied")`
fn notification_title_body_from_reply(reply: &ipc::IpcReply) -> (String, String) {
    let content_type = reply
        .data
        .as_ref()
        .and_then(|d| d["content_type"].as_str())
        .unwrap_or("");
    let preview = reply
        .data
        .as_ref()
        .and_then(|d| d["preview"].as_str())
        .unwrap_or("");

    notification_title_body(content_type, preview)
}

/// Build a rich notification `(title, body)` pair from `content_type` and
/// `preview` strings (the shape returned by `history_page` and `copy_item`).
fn notification_title_body(content_type: &str, preview: &str) -> (String, String) {
    match content_type {
        "text" => {
            let body = build_text_preview_body(preview);
            ("Text Copied".to_owned(), body)
        }
        ct if ct == "image" || ct.starts_with("image/") => {
            ("Image Copied".to_owned(), "Image".to_owned())
        }
        "file" => {
            // preview arrives as "[file: <filename>]" from history_page.
            // Strip the wrapper to show just the filename in the banner.
            let body = if let Some(inner) = preview
                .strip_prefix("[file: ")
                .and_then(|s| s.strip_suffix(']'))
            {
                inner.to_owned()
            } else if !preview.is_empty() {
                preview.to_owned()
            } else {
                "File".to_owned()
            };
            ("File Copied".to_owned(), body)
        }
        _ => {
            // Fallback: unknown or empty content_type.
            let body = build_text_preview_body(preview);
            (
                "Copied".to_owned(),
                if body.is_empty() {
                    "Copied".to_owned()
                } else {
                    body
                },
            )
        }
    }
}

/// Truncate a text `preview` to ~160 chars at a word boundary and append `…`
/// if truncated.  Preserves newlines so multi-line text reads naturally in the
/// notification banner (macOS renders them as line breaks).
fn build_text_preview_body(preview: &str) -> String {
    const MAX_CHARS: usize = 160;
    // Compare CHARS, not bytes: `preview.len()` is the UTF-8 byte length, which
    // for multibyte text (Cyrillic, emoji) overstates the visible length and
    // would truncate far earlier than the intended 160-char budget.
    if preview.chars().count() <= MAX_CHARS {
        return preview.to_owned();
    }
    // Truncate at MAX_CHARS chars (not bytes), preferring a word boundary.
    let truncated: String = preview.chars().take(MAX_CHARS).collect();
    // Walk back to the last whitespace for a clean cut.
    let cut = truncated
        .rfind(|c: char| c.is_whitespace())
        .unwrap_or(MAX_CHARS);
    let chopped = truncated[..cut].trim_end();
    if chopped.is_empty() {
        format!("{}…", truncated.trim_end())
    } else {
        format!("{chopped}…")
    }
}

/// Startup-race + periodic Recent submenu resync.
/// `setup_tray` runs at startup before the daemon socket is necessarily bound,
/// so the Recent submenu often shows a placeholder. This function spawns a
/// background thread that:
///
/// 1. Polls until the daemon responds, then does an initial rebuild.
/// 2. Continues polling every `REFRESH_INTERVAL` so the tray stays current as
///    the user copies things.
/// 3. On each poll, checks whether a new clipboard item appeared (by comparing
///    the most recent item's `wall_time` to the last seen value) and, if so,
///    fires a rich `UNUserNotificationCenter` banner — respecting the daemon's
///    `notify_on_copy` setting.  This bridges background-clipboard-captures
///    (items copied from other apps while the UI is running) to the Tauri
///    bundle so they show the CopyPaste app icon rather than a generic icon.
///
/// The refresh is intentionally cheap: 1-item `history_page` call, only runs
/// while the app is alive, stops after `GIVE_UP_AFTER` of daemon silence.
fn spawn_tray_recent_resync(handle: tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::{Duration, Instant};

    // Grab the stop flag from managed state so the RunEvent::Exit handler can
    // signal this thread to exit cleanly without holding the AppHandle.
    let stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool> = handle
        .try_state::<TrayResyncStop>()
        .map(|s| std::sync::Arc::clone(&s.0))
        .unwrap_or_else(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

    thread::spawn(move || {
        /// How long to wait between refreshes once the daemon is up.
        const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
        /// Poll interval while waiting for the daemon to come up initially.
        const POLL_INTERVAL: Duration = Duration::from_millis(250);
        /// Give up entirely if the daemon never responds within this window.
        const GIVE_UP_AFTER: Duration = Duration::from_secs(30);

        // Phase 1: wait for the daemon to become ready.
        let deadline = Instant::now() + GIVE_UP_AFTER;
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            if Instant::now() >= deadline {
                tracing::warn!("tray Recent re-sync: daemon not ready after 30 s — giving up");
                return;
            }

            // A successful, ok=true history_page reply is the readiness signal.
            let ready = ipc::call(
                "history_page",
                serde_json::json!({ "limit": 1, "offset": 0 }),
            )
            .map(|r| r.ok)
            .unwrap_or(false);

            if ready {
                break;
            }
            thread::sleep(POLL_INTERVAL);
        }

        // Seed the "last seen" wall_time so the first poll doesn't fire a
        // spurious notification for an item that was already in history before
        // the app launched.
        let mut last_seen_wall_time: i64 = {
            ipc::call(
                "history_page",
                serde_json::json!({ "limit": 1, "offset": 0 }),
            )
            .ok()
            .and_then(|r| r.data)
            .and_then(|d| d["items"].as_array().and_then(|a| a.first().cloned()))
            .and_then(|item| item["wall_time"].as_i64())
            .unwrap_or(0)
        };

        // Phase 2: rebuild now and then periodically; exit when stop flag is set.
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            if let Err(e) = rebuild_recent_submenu(&handle) {
                tracing::warn!("tray Recent re-sync: rebuild failed: {e}");
            }

            // Check for a background-captured item (clipboard copy from another
            // app).  Query the most recent item and compare its wall_time.
            // If it is newer AND the daemon config has notify_on_copy enabled,
            // fire a rich UNUserNotificationCenter banner.
            check_and_notify_new_capture(&mut last_seen_wall_time);

            thread::sleep(REFRESH_INTERVAL);
        }
    });
}

/// Poll for the most recent clipboard item and fire a notification if a new
/// background capture appeared since the last check.
///
/// `last_seen` is updated in-place so subsequent calls only notify once per
/// item.  Respects the daemon's `notify_on_copy` setting.
fn check_and_notify_new_capture(last_seen: &mut i64) {
    // Fetch the single most-recent item from history_page (limit=1).
    let reply = match ipc::call(
        "history_page",
        serde_json::json!({ "limit": 1, "offset": 0 }),
    ) {
        Ok(r) if r.ok => r,
        _ => return,
    };

    let item = match reply
        .data
        .as_ref()
        .and_then(|d| d["items"].as_array())
        .and_then(|a| a.first())
    {
        Some(i) => i.clone(),
        None => return,
    };

    let wall_time = match item["wall_time"].as_i64() {
        Some(t) => t,
        None => return,
    };

    if wall_time <= *last_seen {
        return; // no new item
    }

    *last_seen = wall_time;

    // Check notify_on_copy setting before firing.
    let notify_enabled = ipc::call("get_config", serde_json::json!({}))
        .ok()
        .and_then(|r| r.data)
        .and_then(|d| d["notify_on_copy"].as_bool())
        .unwrap_or(false);

    if !notify_enabled {
        return;
    }

    let content_type = item["content_type"].as_str().unwrap_or("").to_owned();
    let preview = item["preview"].as_str().unwrap_or("").to_owned();
    let (title, body) = notification_title_body(&content_type, &preview);

    #[cfg(target_os = "macos")]
    post_un_notification(title, body);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body);
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
