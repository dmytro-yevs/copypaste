//! Persisted UI configuration — shortcut, launch-at-login, popup position,
//! allow-screenshots — plus the Tauri managed state wrappers and the Tauri
//! commands that read/write those settings.

use std::sync::Mutex;
use tauri::{Manager, State};

pub(crate) const DEFAULT_POPUP_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
/// Config filename stored in the Tauri app-config directory.
pub(crate) const CONFIG_FILE: &str = "ui-config.json";

// ---------------------------------------------------------------------------
// Shortcut + launch + position config — persisted to JSON
// ---------------------------------------------------------------------------

/// Popup position mode.  Variants must match the string values the React
/// layer sends/receives so they are serialised as lowercase strings.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PopupPosition {
    /// Show near the mouse cursor (Maccy default).
    #[default]
    Cursor,
    /// Center of the active screen.
    Center,
    /// Below the tray / menu-bar icon (top-right of the primary display).
    Menubar,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct UiConfig {
    /// Per-field default so a config written by a different version (or one
    /// missing this key) still loads instead of silently resetting every
    /// setting back to defaults.
    #[serde(default = "default_popup_shortcut")]
    pub(crate) popup_shortcut: String,
    /// Auto-start CopyPaste at macOS login.  Defaults to `true` so a fresh
    /// install is convenient out-of-the-box; can be disabled in Settings.
    #[serde(default = "default_launch_at_login")]
    pub(crate) launch_at_login: bool,
    /// Where to position the quick-paste popup when shown.
    #[serde(default)]
    pub(crate) popup_position: PopupPosition,
    /// CopyPaste-6uy9: when `true`, screenshot / screen-recording capture is
    /// ALLOWED (content protection is disabled).  Default `false` = protection
    /// ON, matching the previous hard-coded behaviour (PG-25 / CopyPaste-13a3).
    ///
    /// Named "allow_screenshots" so the positive value (true) maps to the user
    /// action ("allow screenshots") rather than a double-negative.
    #[serde(default = "default_allow_screenshots")]
    pub(crate) allow_screenshots: bool,
}

pub(crate) fn default_launch_at_login() -> bool {
    true
}

pub(crate) fn default_popup_shortcut() -> String {
    DEFAULT_POPUP_SHORTCUT.to_string()
}

/// Default for `allow_screenshots`: `false` = protection ON (PG-25 default).
pub(crate) fn default_allow_screenshots() -> bool {
    false
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            popup_shortcut: DEFAULT_POPUP_SHORTCUT.to_string(),
            launch_at_login: true,
            popup_position: PopupPosition::default(),
            allow_screenshots: false,
        }
    }
}

pub(crate) fn config_path(handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    handle
        .path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(CONFIG_FILE))
}

pub(crate) fn load_ui_config(handle: &tauri::AppHandle) -> UiConfig {
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

pub(crate) fn save_ui_config(handle: &tauri::AppHandle, cfg: &UiConfig) -> Result<(), String> {
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

pub(crate) struct CurrentShortcut(pub(crate) Mutex<String>);

/// Current popup-position mode.  Wrapped in Mutex so commands can update it
/// without reloading the full config from disk on every call.
pub(crate) struct CurrentPopupPosition(pub(crate) Mutex<PopupPosition>);

/// CopyPaste-6uy9: persisted allow-screenshots preference.  Wrapped in Mutex
/// so `set_allow_screenshots` can update it without touching the full config.
/// `true` = screenshots allowed (content protection OFF).
/// `false` = protection ON (PG-25 default).
pub(crate) struct AllowScreenshots(pub(crate) Mutex<bool>);

// ---------------------------------------------------------------------------
// Launch-at-login
//
// CopyPaste-8ebg.53: launch_at_login defaulted to `true` with no Settings UI
// control to disable it. get_launch_at_login/set_launch_at_login below close
// that gap — CopyPaste-loyk.7 is superseded by this pair, mirroring the
// get_allow_screenshots/set_allow_screenshots pattern (persist to
// ui-config.json, apply the OS-level effect immediately).
// ---------------------------------------------------------------------------

/// Idempotently sync the OS launch-agent state with the desired value.
///
/// Called both at startup (to enforce the persisted preference) and by
/// `set_launch_at_login`.  Errors are logged but not surfaced — a failure to
/// register/deregister a LaunchAgent is non-fatal for the running app.
pub(crate) fn apply_launch_at_login(handle: &tauri::AppHandle, enabled: bool) {
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

/// Return the persisted launch-at-login preference.
///
/// Reads from `ui-config.json` rather than the OS autolaunch manager so the
/// value reflects what the user last set even if the OS-level registration
/// is momentarily out of sync (e.g. failed silently — see
/// `apply_launch_at_login`).
#[tauri::command]
pub(crate) fn get_launch_at_login(handle: tauri::AppHandle) -> bool {
    load_ui_config(&handle).launch_at_login
}

/// Enable or disable launch-at-login and persist the preference.
///
/// Mirrors `set_allow_screenshots`: apply the OS-level effect first (best
/// effort — logged, not surfaced, since a LaunchAgent registration failure
/// is non-fatal for the running app), then persist to `ui-config.json`.
#[tauri::command]
pub(crate) fn set_launch_at_login(enabled: bool, handle: tauri::AppHandle) -> Result<(), String> {
    apply_launch_at_login(&handle, enabled);

    let mut cfg = load_ui_config(&handle);
    cfg.launch_at_login = enabled;
    save_ui_config(&handle, &cfg)
}

// ---------------------------------------------------------------------------
// Tauri commands — shortcut management
// ---------------------------------------------------------------------------

/// Return the currently configured popup-shortcut accelerator string.
#[tauri::command]
pub(crate) fn get_popup_shortcut(state: State<'_, CurrentShortcut>) -> String {
    state.0.lock().expect("mutex poisoned").clone()
}

/// Return the built-in default popup-shortcut accelerator string.
///
/// CopyPaste-sqw0: this is the Rust-side single source of truth for the
/// default shortcut.  The TypeScript layer (`SettingsView.tsx`) fetches this
/// at load time via `getDefaultPopupShortcut()` so the TS constant and the
/// Rust constant cannot silently drift.  The TS file still carries a hardcoded
/// fallback (`DEFAULT_POPUP_SHORTCUT = "CmdOrCtrl+Shift+V"`) used only while
/// the IPC call is in-flight (initial render) — see the sqw0 cross-reference
/// comment there for details.
#[tauri::command]
pub(crate) fn get_default_popup_shortcut() -> &'static str {
    DEFAULT_POPUP_SHORTCUT
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
pub(crate) fn set_popup_shortcut(
    accelerator: String,
    handle: tauri::AppHandle,
    state: State<'_, CurrentShortcut>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let old = {
        let guard = state.0.lock().expect("mutex poisoned");
        guard.clone()
    };

    // CopyPaste-8ebg.24: register the NEW shortcut BEFORE touching the old
    // one. The previous order unregistered `old` first and only then tried
    // to register `accelerator`; if that registration failed (OS-reserved
    // combo, no CGEventTap fallback), the user was left with the old
    // shortcut unregistered AND the new one not registered — zero working
    // hotkeys, while the UI still displayed the old accelerator as active.
    // Registering first means a failed rebind leaves `old` untouched and
    // still working.
    if old == accelerator {
        // No-op rebind: the accelerator is unchanged, so there is nothing to
        // register/unregister (re-registering the same string can error with
        // "already registered" on some backends).
        return Ok(());
    }

    // Fix #2: attempt to register via the plugin.  On macOS, OS-reserved or
    // shift+alt combos (e.g. Alt+Shift+Q which produces "Œ") may fail here.
    // When the CGEventTap is active it will handle the shortcut anyway, so we
    // treat the plugin registration error as a non-fatal warning in that case.
    let plugin_result = crate::register_popup_shortcut(&handle, &accelerator);
    #[cfg(target_os = "macos")]
    {
        let tap_active = handle
            .try_state::<crate::popup::TapActive>()
            .map(|s| *s.0.lock().expect("mutex poisoned"))
            .unwrap_or(false);
        if !tap_active {
            // No tap — plugin must succeed. `old` is still registered at
            // this point, so on failure the user keeps a working hotkey.
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

    // New shortcut is registered (or accepted via the CGEventTap fallback) —
    // now it is safe to unregister the old one. Best-effort: log a warning
    // when this fails so a ghost shortcut registration doesn't go unnoticed.
    if let Err(e) = handle.global_shortcut().unregister(old.as_str()) {
        tracing::warn!(
            "set_popup_shortcut: failed to unregister old shortcut '{}': {e} \
             (ghost registration possible — old hotkey may still be active)",
            old
        );
    }

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
    crate::event_tap::update_tap_shortcut(&accelerator);

    Ok(())
}

// ---------------------------------------------------------------------------
// Allow-screenshots commands (CopyPaste-6uy9)
// ---------------------------------------------------------------------------

/// Return the current allow-screenshots preference.
/// `true` = screenshots allowed (content protection OFF).
/// `false` = protection ON (default, PG-25).
#[tauri::command]
pub(crate) fn get_allow_screenshots(state: State<'_, AllowScreenshots>) -> bool {
    *state.0.lock().expect("mutex poisoned")
}

/// Enable or disable screenshot / screen-recording protection for all windows.
///
/// `allow = true`  — call set_content_protected(false) on main + popup windows
///                   so screen-capture tools can see CopyPaste.
/// `allow = false` — re-enable protection (default, PG-25 behaviour).
///
/// The preference is persisted to `ui-config.json` and applied immediately to
/// any open windows so the user does not have to restart.
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(crate) fn set_allow_screenshots(
    allow: bool,
    handle: tauri::AppHandle,
    state: State<'_, AllowScreenshots>,
) -> Result<(), String> {
    // Update in-memory state first.
    {
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = allow;
    }

    // Apply to every open window immediately (non-fatal per-window).
    #[cfg(target_os = "macos")]
    {
        let protected = !allow;
        for label in &["main", "popup"] {
            if let Some(win) = handle.get_webview_window(label) {
                if let Err(e) = win.set_content_protected(protected) {
                    tracing::warn!(
                        "CopyPaste-6uy9: set_content_protected({protected}) on {label} failed: {e}"
                    );
                }
            }
        }
    }

    // Persist to config.
    let mut cfg = load_ui_config(&handle);
    cfg.allow_screenshots = allow;
    save_ui_config(&handle, &cfg)
}
