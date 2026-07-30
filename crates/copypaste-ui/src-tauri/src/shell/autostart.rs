//! Launch at login.
//!
//! `tauri-plugin-autostart` rather than writing a `LaunchAgent` plist by hand
//! (CLAUDE.md rule 1). What that buys, concretely: the plist lives in
//! `~/Library/LaunchAgents`, has to name the executable's *current* path, has
//! to be removed rather than rewritten when disabled, and the same feature on
//! any other platform is a completely different mechanism. The plugin owns all
//! of that behind `enable` / `disable` / `is_enabled`.
//!
//! # Why `LaunchAgent` and not `AppleScript`
//!
//! The plugin offers two macOS strategies. `AppleScript` drives System Events
//! to add a Login Item, which means sending Apple events to another process —
//! and that is an Automation TCC grant, held against a cdhash that changes on
//! every ad-hoc-signed build (ADR-0001). It would be revoked on every update,
//! for a feature the user set once and expects to stay set.
//!
//! `LaunchAgent` writes a plist the user owns. No TCC, no prompt, nothing to
//! revoke. It is the default, and it is named explicitly here so that a future
//! edit has to argue with this comment rather than flip a default.
//!
//! # What autostarts
//!
//! The **app**, not the daemon. The daemon is installed as a separate Homebrew
//! formula (ADR-0001) and manages its own lifecycle; the app launching it would
//! mean two components disagreeing about who owns the process. On first run
//! with no daemon the UI already has a state for that, and it is actionable.
//!
//! # Unverified
//!
//! Not run. No macOS host.

use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Install the plugin.
///
/// `args` is `None`: the app takes no startup flags, and a launch-at-login
/// entry that passes arguments the app does not understand is a silent failure
/// on every boot.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)
}

/// Whether the app is set to launch at login.
///
/// `false` on any failure. A plist that cannot be read is reported as "off",
/// which is the state the user can act on — the toggle then turns it on and
/// either works or reports a real error.
pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Turn launch-at-login on or off.
///
/// The error is `&'static str` so no path can reach the user: the plugin's own
/// error renders `~/Library/LaunchAgents/...` on a write failure, and that
/// discloses the username (CLAUDE.md rule 4).
pub fn set_enabled<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
) -> std::result::Result<(), &'static str> {
    let manager = app.autolaunch();
    let outcome = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    outcome.map_err(|_| {
        if enabled {
            "CopyPaste couldn't be set to open at login."
        } else {
            "CopyPaste couldn't be stopped from opening at login."
        }
    })
}
