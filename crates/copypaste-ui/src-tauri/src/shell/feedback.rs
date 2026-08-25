//! One preference gate for successful copy and capture feedback.

use tauri::{AppHandle, Manager as _, Runtime};

use crate::backend::{Backend as _, SelectedBackend};
use crate::capture::{CaptureControl as _, SelectedCapture};

/// Queue feedback for an operation that has already succeeded.
pub fn success<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let enabled = {
            let backend = app.state::<SelectedBackend>();
            match backend.get_config().await {
                Ok(applied) => applied.config.sound_on_copy,
                Err(_) => false,
            }
        };
        if !enabled {
            return;
        }
        let feedback = app.state::<SelectedCapture>();
        if let Err(error) = feedback.play_feedback() {
            tracing::debug!(%error, "copy feedback is unavailable");
        }
    });
}
