//! Launch at login, through `tauri-plugin-autostart` rather than a plist or a
//! registry write of our own (CLAUDE.md rule 1). Three platforms, three
//! mechanisms, one `enable` / `disable` / `is_enabled`.
//!
//! macOS takes the plugin's `LaunchAgent` strategy and never `AppleScript`:
//! driving System Events is an Automation TCC grant held against a cdhash that
//! changes on every ad-hoc-signed build (ADR-0001), so it would be revoked on
//! every update for a setting the user made once. Named here so a future edit
//! has to argue with this rather than flip a default.
//!
//! Windows takes the `Run` key. `auto-launch` also mirrors the
//! `StartupApproved\Run` flag, so switching CopyPaste off in Task Manager's
//! Startup tab reads back as off here instead of as a toggle that claims to be
//! on and does nothing. It writes the executable path *unquoted*
//! (`auto-launch-0.5.0/src/windows.rs`), so a profile name containing a space
//! makes Windows probe `C:\Users\Ada.exe` before the real target; it still
//! starts, and those prefixes are not writable by a standard user.
//!
//! What autostarts is the **app**, not the daemon. The daemon owns its own
//! lifecycle (ADR-0001, ADR-0004) and two owners of one process is the worse
//! failure; a first run with no daemon already has an actionable UI state.
//!
//! Unverified: never run on macOS or Windows.

use tauri::{AppHandle, Runtime};

#[cfg(not(target_os = "android"))]
use tauri::Emitter as _;
#[cfg(not(target_os = "android"))]
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};

use crate::backend::BackendError;

/// Emitted after the setting changes. The tray menu and the Settings row are
/// separate readers of one system fact, and only the writer knows it moved.
pub const EVENT_CHANGED: &str = "autostart-changed";

pub const MSG_ENABLE_FAILED: &str = "CopyPaste couldn't be set to open at login.";
pub const MSG_DISABLE_FAILED: &str = "CopyPaste couldn't be stopped from opening at login.";

/// Install the plugin.
///
/// `args` is `None`: the app takes no startup flags, and a launch-at-login
/// entry that passes arguments the app does not understand is a silent failure
/// on every boot.
#[cfg(not(target_os = "android"))]
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)
}

/// Whether the app is set to launch at login.
///
/// `false` on any failure. An entry that cannot be read is reported as "off",
/// which is the state the user can act on — the toggle then turns it on and
/// either works or reports a real error.
#[cfg(not(target_os = "android"))]
pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[cfg(target_os = "android")]
pub fn is_enabled<R: Runtime>(_app: &AppHandle<R>) -> bool {
    false
}

/// Turn launch-at-login on or off, then tell every surface that shows it.
///
/// The messages are `&'static str` so no path can reach the user: the plugin's
/// own error renders `~/Library/LaunchAgents/...` or the registry value it
/// tried to write, and both disclose the username (CLAUDE.md rule 4).
#[cfg(not(target_os = "android"))]
pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), BackendError> {
    let manager = app.autolaunch();
    let outcome = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    outcome.map_err(|_| {
        BackendError::Internal(
            if enabled {
                MSG_ENABLE_FAILED
            } else {
                MSG_DISABLE_FAILED
            }
            .into(),
        )
    })?;
    // The observed state, not `enabled`: on Windows the write can succeed while
    // Task Manager's override still holds the app off, and a menu that reported
    // the request rather than the result would then be lying.
    let _ = app.emit(EVENT_CHANGED, is_enabled(app));
    Ok(())
}

#[cfg(target_os = "android")]
pub fn set_enabled<R: Runtime>(_app: &AppHandle<R>, _enabled: bool) -> Result<(), BackendError> {
    Err(BackendError::Unsupported(
        "Opening at login is available on desktop only.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// INV-12: the launch-at-login entry names a path on every platform, and
    /// these are the only sentences that describe failing to write it.
    #[test]
    fn neither_failure_message_can_carry_a_path() {
        for message in [MSG_ENABLE_FAILED, MSG_DISABLE_FAILED] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
