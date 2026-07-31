//! The diagnostics command.
//!
//! One call rather than two, because the panel it feeds has to say what is
//! running *and* what that thing has dropped, and two rounds can disagree: a
//! service that stops between them renders as running with no counters, which
//! is the state the panel exists to make legible.

use tauri::State;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::service::diagnostics::Diagnostics;
use crate::service::Supervisor;

/// What is running, what it has refused or dropped, and a block to paste.
///
/// Never fails on an absent service: "not answering" is the answer a user
/// opening this panel most often needs, and an error would render as an empty
/// screen where the diagnosis should be.
#[tauri::command]
pub async fn diagnostics(
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> std::result::Result<Diagnostics, BackendError> {
    let service = supervisor.state(backend.inner()).await;
    let status = backend.status().await.ok();
    Ok(Diagnostics::new(service, status))
}
