//! The status command.
//!
//! Its own file rather than an odd one out in `history`, because it is the one
//! command that must answer when nothing else can: the frontend calls it to
//! decide between "everything is fine", "still starting up" and "the daemon
//! isn't running", and the daemon exempts it from its own readiness gate for
//! the same reason (manifest 04 §6.1).

use tauri::State;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::model::UiStatus;

/// Whether the backend is up, and what it is doing.
///
/// `clipboard_backend` is surfaced so a demo cannot be mistaken for the real
/// thing: on Android it reads `android-inprocess` and `capture_running` is
/// false, because that build has no capture loop at all.
#[tauri::command]
pub async fn status(
    backend: State<'_, SelectedBackend>,
) -> std::result::Result<UiStatus, BackendError> {
    backend.status().await
}
