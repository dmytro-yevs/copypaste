//! The "something was captured" notification (parity 18, backlog B-18).
//!
//! The daemon cannot post it. `UNUserNotificationCenter` needs an application
//! bundle and the daemon is a bare executable, so the daemon reports
//! [`copypaste_ipc::EventData::captured`] and the app decides what to do about
//! it (manifest 06 §3.6). Windows divides the same way for its own reason: the
//! plugin sets our AppUserModelID only once the executable is outside
//! `target/debug|release` (`tauri-plugin-notification-2.3.3/src/desktop.rs`),
//! so a toast appears from an installed build and not under `tauri dev`.
//!
//! Native delivery remains release-host evidence. Unit tests exercise the
//! policy gate and Android's explicitly silent notification payload.

use tauri::{AppHandle, Manager as _, Runtime};
use tauri_plugin_notification::{NotificationExt as _, PermissionState};

use super::window;
use crate::backend::{Backend as _, SelectedBackend};

/// No clipboard content, ever. The change event carries none by design, and a
/// notification is drawn where nothing this app owns can redact it — the same
/// argument `tray::recent` makes about menu labels.
pub const TITLE: &str = "Saved to CopyPaste";
pub const BODY: &str = "What you just copied is in your history.";
pub const COPY_TITLE: &str = "Copied from CopyPaste";
pub const COPY_BODY: &str = "The clipping is ready to paste.";

/// Something was captured. Post the notification, if the user asked for one and
/// is not already looking at the app.
pub fn on_capture<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Asked first because it is local, and answering it here spares an IPC
        // round trip on every capture made while the popover is open.
        let foreground = is_foreground(&app);
        if foreground {
            return;
        }
        let enabled = {
            let backend = app.state::<SelectedBackend>();
            match backend.get_config().await {
                Ok(applied) => applied.config.notify_on_copy,
                // A service that cannot be asked has not said yes.
                Err(_) => false,
            }
        };
        if !should_post(enabled, foreground) {
            return;
        }
        // `show` goes to the platform and blocks; the caller is the change
        // stream's task.
        let _ = tauri::async_runtime::spawn_blocking(move || post(&app, TITLE, BODY)).await;
    });
}

/// A tray selection is a copy just as a history-row selection is. It needs the
/// same native notification without exposing the clipping.
pub fn on_recent_copy<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let config = {
            let backend = app.state::<SelectedBackend>();
            match backend.get_config().await {
                Ok(applied) => applied.config,
                Err(_) => return,
            }
        };
        if !config.notify_on_copy {
            return;
        }
        let _ =
            tauri::async_runtime::spawn_blocking(move || post(&app, COPY_TITLE, COPY_BODY)).await;
    });
}

/// The whole decision, as a pure function, so it is testable on any host.
pub fn should_post(enabled: bool, foreground: bool) -> bool {
    enabled && !foreground
}

/// Whether the user is already looking at the app.
fn is_foreground<R: Runtime>(app: &AppHandle<R>) -> bool {
    window::main_window(app).is_some_and(|window| window.is_focused().unwrap_or(false))
}

fn post<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    let notifier = app.notification();

    // Inert on the desktop, where both calls answer `Granted` — macOS decides at
    // post time. It is Android 13's `POST_NOTIFICATIONS` that needs asking, and
    // asking here rather than at startup means the prompt only appears for a
    // user who has switched the setting on.
    let mut state = notifier
        .permission_state()
        .unwrap_or(PermissionState::Denied);
    if state != PermissionState::Granted {
        state = notifier
            .request_permission()
            .unwrap_or(PermissionState::Denied);
    }
    if state != PermissionState::Granted {
        return;
    }

    if let Err(error) = show_silent(app, title, body) {
        tracing::debug!(%error, "the capture notification was not posted");
    }
}

#[cfg(target_os = "android")]
fn show_silent<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> crate::backend::Result<()> {
    app.state::<crate::capture::SelectedCapture>()
        .post_silent_notification(title, body)
}

#[cfg(not(target_os = "android"))]
fn show_silent<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> crate::backend::Result<()> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|_| crate::backend::BackendError::internal("notification delivery failed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_posted_unless_the_user_asked() {
        assert!(!should_post(false, false));
        assert!(should_post(true, false));
    }

    #[test]
    fn notification_delivery_is_not_coupled_to_copy_feedback() {
        let source = include_str!("notify.rs");
        assert!(!source.contains(&["sound", "_on_copy"].concat()));
        assert!(!source.contains(&[".", "sound", "("].concat()));
    }

    /// The setting says "while CopyPaste is in the background", and a
    /// notification about a copy the user just watched land is noise.
    #[test]
    fn nothing_is_posted_while_the_user_is_looking_at_the_app() {
        assert!(!should_post(true, true));
    }

    /// A notification is unblurrable and often on screen during a screen share.
    /// Whatever was copied must not be in it, and neither must a path (INV-12).
    #[test]
    fn the_notification_carries_neither_content_nor_a_path() {
        for text in [TITLE, BODY, COPY_TITLE, COPY_BODY] {
            assert!(!text.contains('/'), "{text}");
            assert!(!text.contains('\\'), "{text}");
        }
    }
}
