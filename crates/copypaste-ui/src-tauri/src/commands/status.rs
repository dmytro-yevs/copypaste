//! The status command.
//!
//! Its own file rather than an odd one out in `history`, because it is the one
//! command that must answer when nothing else can: the frontend calls it to
//! decide between "everything is fine", "still starting up" and "the daemon
//! isn't running", and the daemon exempts it from its own readiness gate for
//! the same reason (manifest 04 §6.1).

use tauri::{AppHandle, State};

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::UiStatus;

#[cfg(any(target_os = "android", test))]
use crate::capture::model::CaptureHealth;
#[cfg(target_os = "android")]
use crate::capture::{CaptureControl, SelectedCapture};
#[cfg(target_os = "android")]
use tauri::Manager as _;

/// Whether the backend is up, and what it is doing.
///
/// `clipboard_backend` is surfaced so a demo cannot be mistaken for the real
/// thing: on Android it reads `android-inprocess`; its background-capture
/// field is derived from the Android listener rather than from the embedded
/// store, which cannot know whether Kotlin is still receiving clipboard events.
#[tauri::command]
pub async fn status(
    app: AppHandle,
    backend: State<'_, SelectedBackend>,
) -> std::result::Result<UiStatus, BackendError> {
    let status = backend.status().await?;

    #[cfg(target_os = "android")]
    {
        let mut status = status;
        let capture = app.state::<SelectedCapture>();
        status.capture_running = android_capture_running(&capture.snapshot());
        return Ok(status);
    }

    #[cfg(not(target_os = "android"))]
    let _ = app;

    Ok(status)
}

#[cfg(any(target_os = "android", test))]
fn android_capture_running(snapshot: &crate::capture::model::CaptureSnapshot) -> bool {
    matches!(snapshot.health, CaptureHealth::Working)
}

#[cfg(test)]
mod tests {
    use super::android_capture_running;
    use crate::capture::model::{CaptureModel, ReadOutcome, ShizukuProbe};

    #[test]
    fn android_status_only_reports_background_capture_after_a_real_listener_read() {
        let mut model = CaptureModel::android();
        assert!(!android_capture_running(&model.snapshot()));

        model.set_enabled(true);
        model.set_probe(ShizukuProbe {
            supported: true,
            installed: true,
            running: true,
            permission: true,
            ..ShizukuProbe::default()
        });
        model.record_armed(true);
        assert!(!android_capture_running(&model.snapshot()));

        model.record_read(ReadOutcome::Succeeded, false, 1);
        assert!(android_capture_running(&model.snapshot()));
    }
}
