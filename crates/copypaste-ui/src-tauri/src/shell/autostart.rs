//! Launch at login through `tauri-plugin-autostart`, which owns the distinct
//! macOS and Windows mechanisms behind one interface.
//!
//! macOS uses `LaunchAgent`, never `AppleScript`: driving System Events needs an
//! Automation TCC grant tied to a cdhash that changes on ad-hoc-signed builds.
//!
//! Windows uses the `Run` key and mirrors `StartupApproved\Run`, so disabling
//! CopyPaste through Task Manager reads back as disabled here.
//!
//! The app autostarts, not the daemon, whose lifecycle has a separate owner
//! (ADR-0001, ADR-0004). Unverified on macOS and Windows.

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
/// Static messages prevent the plugin's path-bearing errors from disclosing a
/// username through a LaunchAgent or registry path.
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
