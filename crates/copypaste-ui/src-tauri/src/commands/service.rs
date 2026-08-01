//! Starting, restarting and reporting the background service (ADR-0004).
//!
//! Three commands and one window verb. They are the app's half of "opening the
//! app starts the daemon, quitting stops it" — the stopping half is not a
//! command, because it is not a thing the user asks for: it happens on
//! `RunEvent::Exit` in `crate::run`.

use tauri::{State, WebviewWindow};

use crate::backend::{BackendError, SelectedBackend};
use crate::service::{ServiceState, Supervisor};

type Result<T> = std::result::Result<T, BackendError>;

/// What the background service is doing, without changing it.
///
/// Polled by the offline screen, which needs to tell four situations apart:
/// nothing installed, installed and stopped, running, and running on a version
/// this app did not ship with.
#[tauri::command]
pub async fn service_state(
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> Result<ServiceState> {
    Ok(supervisor.state(backend.inner()).await)
}

/// Start it if it is not running, and wait until it answers.
///
/// Resolves only once the service is actually reachable, so the screen that
/// called it can invalidate its queries and find data rather than a second
/// offline state.
#[tauri::command]
pub async fn start_service(
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> Result<ServiceState> {
    supervisor.start(backend.inner()).await
}

/// Stop what this app started and start it again.
///
/// Refuses when the running service is not ours — see ADR-0004.
#[tauri::command]
pub async fn restart_service(
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> Result<ServiceState> {
    supervisor.restart(backend.inner()).await
}

/// Hide the invoking window from the frontend, through the backend's hide path.
///
/// INV-25: the frontend must never reach for the window itself. On macOS the
/// app is an `Accessory`, so hiding hands activation back to whatever the user
/// was in — which is the whole point of the quick-copy flow, since the app
/// never synthesises a paste (ADR-0001) and the user presses ⌘V themselves.
///
/// Deliberately infallible: a hide that could fail would leave the caller
/// deciding what to do about a window it must not touch.
#[tauri::command]
pub fn hide_window(window: WebviewWindow) {
    crate::shell::window::hide_window(&window);
}

/// Open the full application surface from Quick Paste's settings affordance.
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) {
    crate::shell::window::show_main(&app);
}
