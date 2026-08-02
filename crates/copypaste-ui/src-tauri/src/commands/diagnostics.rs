//! The diagnostics command.
//!
//! One call rather than two, because the panel it feeds has to say what is
//! running *and* what that thing has dropped, and two rounds can disagree: a
//! service that stops between them renders as running with no counters, which
//! is the state the panel exists to make legible.

use std::io::Write;
#[cfg(not(target_os = "android"))]
use std::os::unix::fs::OpenOptionsExt as _;

use tauri::{AppHandle, Runtime, State};
use tauri_plugin_fs::{FsExt as _, OpenOptions};

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::commands::transfer::save_panel_file;
use crate::service::diagnostics::{Diagnostics, HistoryRead};
use crate::service::Supervisor;

const REPORT_NAME: &str = "copypaste-diagnostics.txt";
const BUNDLE_NAME: &str = "copypaste-support-bundle.txt";
const MSG_REPORT_NOT_WRITTEN: &str = "The diagnostic report couldn't be saved.";
const MSG_BUNDLE_NOT_WRITTEN: &str = "The support bundle couldn't be saved.";
const MSG_LOGS_NOT_READ: &str = "Runtime events couldn't be loaded.";

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
    let history_read = history_read(backend.inner()).await;
    Ok(Diagnostics::new(service, status, history_read))
}

/// Read a bounded, redacted page of runtime events for the in-app log viewer.
///
/// The runtime-log crate accepts only its own message-only format. Android has
/// no daemon, so it can return app-process events only rather than presenting
/// an empty source as if a background service should exist.
#[tauri::command]
pub async fn runtime_log_events<R: Runtime>(
    app: AppHandle<R>,
    query: copypaste_runtime_log::RuntimeLogQuery,
) -> std::result::Result<copypaste_runtime_log::RuntimeLogPage, BackendError> {
    let log_dir = crate::runtime_log_dir(&app)
        .map_err(|_| BackendError::Internal(MSG_LOGS_NOT_READ.into()))?;
    copypaste_runtime_log::list(&log_dir, &query, !cfg!(target_os = "android"))
        .map_err(|_| BackendError::Internal(MSG_LOGS_NOT_READ.into()))
}

/// Export the same redacted support report the UI can show and copy.
///
/// `FilePath` deliberately stays on the Rust side. On Android it is a
/// `content://` URI that `tauri-plugin-fs` opens through the system document
/// provider; on macOS it is the path chosen in the native save panel.
#[tauri::command]
pub async fn export_diagnostics_report<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> std::result::Result<bool, BackendError> {
    let service = supervisor.state(backend.inner()).await;
    let status = backend.status().await.ok();
    let history_read = history_read(backend.inner()).await;
    let report = Diagnostics::new(service, status, history_read).report;

    let Some(dest) = save_panel_file(&app, REPORT_NAME).await else {
        return Ok(false);
    };

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(not(target_os = "android"))]
    options.mode(0o600);
    let mut file = app
        .fs()
        .open(dest, options)
        .map_err(|_| BackendError::Internal(MSG_REPORT_NOT_WRITTEN.into()))?;
    file.write_all(report.as_bytes())
        .map_err(|_| BackendError::Internal(MSG_REPORT_NOT_WRITTEN.into()))?;
    Ok(true)
}

/// Export the current diagnostics separately from redacted app and service
/// runtime events. The picker and destination stay native on both platforms.
#[tauri::command]
pub async fn export_support_bundle<R: Runtime>(
    app: AppHandle<R>,
    backend: State<'_, SelectedBackend>,
    supervisor: State<'_, Supervisor>,
) -> std::result::Result<bool, BackendError> {
    let service = supervisor.state(backend.inner()).await;
    let status = backend.status().await.ok();
    let history_read = history_read(backend.inner()).await;
    let diagnostics = Diagnostics::new(service, status, history_read).report;
    let log_dir = crate::runtime_log_dir(&app)
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?;
    let app_events = copypaste_runtime_log::export(&log_dir, copypaste_runtime_log::Process::App)
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?;
    // Android has no daemon: capture, storage and peer sync run in this app
    // process and are therefore already in `app_events`. Do not write a blank
    // daemon section that implies there ought to be a second log source.
    let daemon_events = if cfg!(target_os = "android") {
        None
    } else {
        Some(
            copypaste_runtime_log::export(&log_dir, copypaste_runtime_log::Process::Daemon)
                .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?,
        )
    };
    let bundle = format_support_bundle(&diagnostics, &app_events, daemon_events.as_deref());

    let Some(dest) = save_panel_file(&app, BUNDLE_NAME).await else {
        return Ok(false);
    };
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(not(target_os = "android"))]
    options.mode(0o600);
    let mut file = app
        .fs()
        .open(dest, options)
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?;
    file.write_all(bundle.as_bytes())
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?;
    Ok(true)
}

/// Checks the same read path used by History without retaining an item or
/// serialising its content into diagnostics.
async fn history_read(backend: &impl Backend) -> HistoryRead {
    match backend.list(1, None).await {
        Ok(_) => HistoryRead::Readable,
        Err(error) => HistoryRead::Failed {
            code: error.ui_error().code,
        },
    }
}

fn format_support_bundle(
    diagnostics: &str,
    app_events: &str,
    daemon_events: Option<&str>,
) -> String {
    let mut bundle = format!(
        "CopyPaste support bundle\n\n=== Diagnostics ===\n{diagnostics}\n=== App runtime events ===\n{app_events}"
    );
    if let Some(daemon_events) = daemon_events {
        bundle.push_str("\n=== Background service runtime events ===\n");
        bundle.push_str(daemon_events);
    }
    bundle
}

#[cfg(test)]
mod tests {
    use super::format_support_bundle;

    #[test]
    fn support_bundle_keeps_diagnostics_and_runtime_events_distinct() {
        let bundle = format_support_bundle("app: 2", "app started\n", Some("service started\n"));
        assert!(bundle.contains("=== Diagnostics ===\napp: 2"));
        assert!(bundle.contains("=== App runtime events ===\napp started"));
        assert!(bundle.contains("=== Background service runtime events ===\nservice started"));
    }

    #[test]
    fn android_bundle_does_not_claim_a_daemon_log() {
        let bundle = format_support_bundle("app: 2", "capture armed\n", None);
        assert!(bundle.contains("=== App runtime events ===\ncapture armed"));
        assert!(!bundle.contains("Background service runtime events"));
    }
}
