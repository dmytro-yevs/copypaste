//! Onboarding permission requests. The OS prompt is raised only from a tap.

use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt as _;

use crate::backend::BackendError;

mod model;
mod policy;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "macos")]
mod macos;

pub use model::{
    OnboardingPermissions, PermissionHost, PermissionId, PermissionItem, PermissionStatus,
};

const MSG_SETTINGS: &str = "CopyPaste couldn't open notification settings.";

pub fn platform() -> PermissionHost {
    #[cfg(target_os = "macos")]
    {
        PermissionHost::Macos
    }
    #[cfg(target_os = "windows")]
    {
        PermissionHost::Windows
    }
    #[cfg(target_os = "android")]
    {
        PermissionHost::Android
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    {
        PermissionHost::Linux
    }
}

pub fn snapshot<R: Runtime>(app: &AppHandle<R>) -> Result<OnboardingPermissions, BackendError> {
    Ok(OnboardingPermissions::assemble(
        platform(),
        notification_status(app)?,
        tile_status(app)?,
    ))
}

pub fn request<R: Runtime>(
    app: &AppHandle<R>,
    id: PermissionId,
) -> Result<OnboardingPermissions, BackendError> {
    match id {
        PermissionId::Notifications => request_notifications(app)?,
        PermissionId::Tile => request_tile(app)?,
    }
    snapshot(app)
}

pub fn open_settings<R: Runtime>(
    app: &AppHandle<R>,
    id: PermissionId,
) -> Result<OnboardingPermissions, BackendError> {
    match id {
        PermissionId::Notifications => open_notification_settings(app)?,
        PermissionId::Tile => {
            // The tile prompt is the OS sheet. Settings has no dedicated pane.
            request_tile(app)?;
        }
    }
    snapshot(app)
}

fn notification_status<R: Runtime>(app: &AppHandle<R>) -> Result<PermissionStatus, BackendError> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        Ok(policy::notification_status(macos::authorization()?))
    }
    #[cfg(target_os = "android")]
    {
        Ok(android::notification_status(app)?)
    }
    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    {
        let _ = app;
        Ok(PermissionStatus::NotRequired)
    }
}

fn tile_status<R: Runtime>(app: &AppHandle<R>) -> Result<PermissionStatus, BackendError> {
    #[cfg(target_os = "android")]
    {
        android::tile_status(app)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(PermissionStatus::Unavailable)
    }
}

fn request_notifications<R: Runtime>(app: &AppHandle<R>) -> Result<(), BackendError> {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        macos::request()
    }
    #[cfg(target_os = "android")]
    {
        android::request_notifications(app)
    }
    #[cfg(not(any(target_os = "macos", target_os = "android")))]
    {
        let _ = app;
        Ok(())
    }
}

fn request_tile<R: Runtime>(app: &AppHandle<R>) -> Result<(), BackendError> {
    #[cfg(target_os = "android")]
    {
        android::request_tile(app)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err(BackendError::Unsupported(
            "The Quick Settings tile is available on Android only.",
        ))
    }
}

fn open_notification_settings<R: Runtime>(app: &AppHandle<R>) -> Result<(), BackendError> {
    #[cfg(target_os = "android")]
    {
        android::open_notification_settings(app)
    }
    #[cfg(target_os = "macos")]
    const URL: &str = "x-apple.systemsettings:com.apple.Notifications-Settings.extension";
    #[cfg(target_os = "windows")]
    const URL: &str = "ms-settings:notifications";
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    {
        let _ = app;
        Err(BackendError::Unsupported(MSG_SETTINGS))
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        app.opener()
            .open_url(URL, None::<&str>)
            .map_err(|_| BackendError::Internal(MSG_SETTINGS.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_request_messages_carry_no_path() {
        assert!(!MSG_SETTINGS.contains('/'), "{MSG_SETTINGS}");
        assert!(!MSG_SETTINGS.contains('\\'), "{MSG_SETTINGS}");
    }

    #[test]
    fn clipboard_capture_is_never_a_blocking_permission() {
        let snapshot = OnboardingPermissions::assemble(
            PermissionHost::Macos,
            PermissionStatus::Denied,
            PermissionStatus::Unavailable,
        );
        assert_eq!(snapshot.clipboard_status, PermissionStatus::NotRequired);
        assert!(!snapshot.notifications.required);
        assert!(!snapshot.tile.required);
    }
}
