//! The diagnostics command.
//!
//! One call rather than two, because the panel it feeds has to say what is
//! running *and* what that thing has dropped, and two rounds can disagree: a
//! service that stops between them renders as running with no counters, which
//! is the state the panel exists to make legible.

use tauri::{AppHandle, Runtime, State};

use crate::backend::{BackendError, SelectedBackend};
use crate::commands::document;
use crate::service::diagnostics::{backend_snapshot, Diagnostics, DIAGNOSTICS_PROBE_BUDGET};
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
    let (service, snapshot) = tokio::join!(
        tokio::time::timeout(DIAGNOSTICS_PROBE_BUDGET, supervisor.state(backend.inner()),),
        backend_snapshot(backend.inner()),
    );
    let service = service.unwrap_or_else(|_| snapshot.service_hint.clone());
    Ok(Diagnostics::new(
        service,
        snapshot.status,
        snapshot.history_read,
    ))
}

/// Read a bounded, redacted page of runtime events for the in-app log viewer.
///
/// The runtime-log crate accepts only its own message-only format. Android has
/// no daemon, so it can return app-process events only rather than presenting
/// an empty source as if a background service should exist.
///
/// On a blocking thread, because the read is one: up to a megabyte of log off
/// disk, decoded and filtered. ADR-0011 promises the runtime log never stalls
/// the app, and doing this on the reactor would break that promise from the
/// read side — with the whole UI, including the panel showing the log, frozen
/// behind it.
#[tauri::command]
pub async fn runtime_log_events<R: Runtime>(
    app: AppHandle<R>,
    query: copypaste_runtime_log::RuntimeLogQuery,
) -> std::result::Result<copypaste_runtime_log::RuntimeLogPage, BackendError> {
    let log_dir = crate::runtime_log_dir(&app)
        .map_err(|_| BackendError::Internal(MSG_LOGS_NOT_READ.into()))?;
    off_reactor(move || copypaste_runtime_log::list(&log_dir, &query, !cfg!(target_os = "android")))
        .await
        .map_err(|_| BackendError::Internal(MSG_LOGS_NOT_READ.into()))
}

/// Run one blocking runtime-log read on a thread that is allowed to block.
///
/// A join failure is folded into the same `io::Error` the read itself returns,
/// so callers keep one failure to map and the panel keeps one sentence.
async fn off_reactor<T: Send + 'static>(
    read: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> std::io::Result<T> {
    tokio::task::spawn_blocking(read)
        .await
        .unwrap_or_else(|_| Err(std::io::Error::other("the runtime log read did not finish")))
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
    let snapshot = backend_snapshot(backend.inner()).await;
    let report = Diagnostics::new(service, snapshot.status, snapshot.history_read).report;

    let Some(dest) = document::save_panel_file(&app, REPORT_NAME).await else {
        return Ok(false);
    };

    document::write_picked(&app, dest, report.as_bytes(), MSG_REPORT_NOT_WRITTEN)?;
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
    let snapshot = backend_snapshot(backend.inner()).await;
    let diagnostics = Diagnostics::new(service, snapshot.status, snapshot.history_read).report;
    let log_dir = crate::runtime_log_dir(&app)
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?;
    // Off the reactor for the reason `runtime_log_events` is, and more so: an
    // export reads every retained file rather than one page of tails.
    let app_events = {
        let log_dir = log_dir.clone();
        off_reactor(move || {
            copypaste_runtime_log::export(&log_dir, copypaste_runtime_log::Process::App)
        })
        .await
        .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?
    };
    // Android has no daemon: capture, storage and peer sync run in this app
    // process and are therefore already in `app_events`. Do not write a blank
    // daemon section that implies there ought to be a second log source.
    let daemon_events = if cfg!(target_os = "android") {
        None
    } else {
        Some(
            off_reactor(move || {
                copypaste_runtime_log::export(&log_dir, copypaste_runtime_log::Process::Daemon)
            })
            .await
            .map_err(|_| BackendError::Internal(MSG_BUNDLE_NOT_WRITTEN.into()))?,
        )
    };
    let bundle = format_support_bundle(&diagnostics, &app_events, daemon_events.as_deref());

    let Some(dest) = document::save_panel_file(&app, BUNDLE_NAME).await else {
        return Ok(false);
    };
    document::write_picked(&app, dest, bundle.as_bytes(), MSG_BUNDLE_NOT_WRITTEN)?;
    Ok(true)
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

    /// ADR-0011's promise, from the read side.
    ///
    /// The regression is silent: calling `list` or `export` directly still
    /// compiles and still returns the right page, and the only symptom is a UI
    /// that stops repainting while a megabyte of log is decoded — on the very
    /// panel that is showing it. Checked against the source because there is no
    /// runtime assertion for "this ran on the reactor".
    #[test]
    fn every_runtime_log_read_goes_through_a_blocking_thread() {
        let source = include_str!("diagnostics.rs");
        let body = source.split_once("#[cfg(test)]").expect("a test module").0;
        for call in [
            "copypaste_runtime_log::list(",
            "copypaste_runtime_log::export(",
        ] {
            for (offset, _) in body.match_indices(call) {
                let before = &body[..offset];
                let enclosing = before
                    .rfind("off_reactor(")
                    .zip(before.rfind("#[tauri::command]"))
                    .is_some_and(|(wrapped, command)| wrapped > command);
                assert!(enclosing, "{call} is not inside an off_reactor call");
            }
        }
    }
}
