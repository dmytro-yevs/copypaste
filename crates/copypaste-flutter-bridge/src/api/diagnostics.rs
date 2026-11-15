//! Runtime diagnostics: a paged view of the redacted app and daemon logs, and
//! the bounded support export. Android has no daemon, so it lists the app only.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use copypaste_runtime_log::{LogLevel, Process, RuntimeEvent, RuntimeLogQuery};

use super::RuntimeError;

const INCLUDES_DAEMON: bool = cfg!(not(target_os = "android"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticProcess {
    App,
    Daemon,
}

#[derive(Debug, Clone)]
pub struct DiagnosticEvent {
    pub timestamp_ms: i64,
    pub level: DiagnosticLevel,
    pub process: DiagnosticProcess,
    pub target: String,
    pub message: String,
}

/// App-side events recorded in the runtime log. A fixed set, never free text,
/// so only these literal messages can reach the log from Dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLogEvent {
    UpdateCheckStarted,
    UpdateUpToDate,
    UpdateAvailable,
    UpdateUnavailable,
    UpdateDownloading,
    UpdateInstalling,
    UpdateRestartRequired,
    UpdateFailed,
}

pub fn record_app_event(event: AppLogEvent) {
    match event {
        AppLogEvent::UpdateCheckStarted => tracing::debug!("update check started"),
        AppLogEvent::UpdateUpToDate => tracing::info!("update check: up to date"),
        AppLogEvent::UpdateAvailable => tracing::info!("update check: an update is available"),
        AppLogEvent::UpdateUnavailable => {
            tracing::info!("update check: updates are unavailable on this installation");
        }
        AppLogEvent::UpdateDownloading => tracing::info!("update download started"),
        AppLogEvent::UpdateInstalling => tracing::info!("update install started"),
        AppLogEvent::UpdateRestartRequired => tracing::info!("update installed; restart required"),
        AppLogEvent::UpdateFailed => tracing::warn!("update check or install failed"),
    }
}

/// One page, newest first. `next_cursor` is opaque and `None` at the end.
#[derive(Debug, Clone)]
pub struct DiagnosticPage {
    pub events: Vec<DiagnosticEvent>,
    pub next_cursor: Option<String>,
}

/// Reads one bounded page; a filter change must restart from `cursor: None`.
pub async fn diagnostic_events(
    cursor: Option<String>,
    level: Option<DiagnosticLevel>,
    process: Option<DiagnosticProcess>,
    query: Option<String>,
    limit: u32,
) -> Result<DiagnosticPage, RuntimeError> {
    let dir = log_dir()?;
    let query = RuntimeLogQuery {
        cursor,
        level: level.map(log_level),
        process: process.map(log_process),
        query,
        limit: limit as usize,
    };
    let page = tokio::task::spawn_blocking(move || {
        copypaste_runtime_log::list(&dir, &query, INCLUDES_DAEMON)
    })
    .await
    .map_err(|_| RuntimeError::internal())?
    .map_err(|_| unavailable())?;
    Ok(DiagnosticPage {
        events: page.events.into_iter().map(event).collect(),
        next_cursor: page.next_cursor,
    })
}

/// Writes the bounded, redacted support text to `path`, replacing it whole.
pub async fn export_diagnostics(path: String) -> Result<(), RuntimeError> {
    let dir = log_dir()?;
    tokio::task::spawn_blocking(move || write_export(&dir, Path::new(&path)))
        .await
        .map_err(|_| RuntimeError::internal())?
        .map_err(|_| RuntimeError {
            code: "diagnostics_export_failed".into(),
            message: "The diagnostics could not be saved.".into(),
        })
}

fn write_export(dir: &Path, path: &Path) -> std::io::Result<()> {
    let mut file = copypaste_fs::PendingFile::create(path)?;
    writeln!(
        file,
        "CopyPaste {} diagnostics ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )?;
    let mut processes = vec![Process::App];
    if INCLUDES_DAEMON {
        processes.push(Process::Daemon);
    }
    for process in processes {
        let name = match process {
            Process::App => "app",
            Process::Daemon => "daemon",
        };
        writeln!(file, "\n== {name} ==")?;
        copypaste_runtime_log::export(dir, process, &mut file)?;
    }
    file.publish()
}

fn log_dir() -> Result<PathBuf, RuntimeError> {
    crate::runtime_log::dir()
        .map(Path::to_path_buf)
        .ok_or_else(unavailable)
}

fn unavailable() -> RuntimeError {
    RuntimeError {
        code: "diagnostics_unavailable".into(),
        message: "Diagnostics are unavailable.".into(),
    }
}

fn event(event: RuntimeEvent) -> DiagnosticEvent {
    DiagnosticEvent {
        timestamp_ms: i64::try_from(event.timestamp_ms).unwrap_or(i64::MAX),
        level: match event.level {
            LogLevel::Error => DiagnosticLevel::Error,
            LogLevel::Warn => DiagnosticLevel::Warn,
            LogLevel::Info => DiagnosticLevel::Info,
            LogLevel::Debug => DiagnosticLevel::Debug,
            LogLevel::Trace => DiagnosticLevel::Trace,
        },
        process: match event.process {
            Process::App => DiagnosticProcess::App,
            Process::Daemon => DiagnosticProcess::Daemon,
        },
        target: event.target,
        message: event.message,
    }
}

fn log_level(level: DiagnosticLevel) -> LogLevel {
    match level {
        DiagnosticLevel::Error => LogLevel::Error,
        DiagnosticLevel::Warn => LogLevel::Warn,
        DiagnosticLevel::Info => LogLevel::Info,
        DiagnosticLevel::Debug => LogLevel::Debug,
        DiagnosticLevel::Trace => LogLevel::Trace,
    }
}

fn log_process(process: DiagnosticProcess) -> Process {
    match process {
        DiagnosticProcess::App => Process::App,
        DiagnosticProcess::Daemon => Process::Daemon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_writes_both_redacted_sections_and_skips_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("app.2026-11-15.000001.log"),
            "100 INFO copypaste::ui: opened /Users/alice/notes.txt\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("daemon.2026-11-15.000001.log"),
            "200 WARN copypaste::sync: peer unreachable\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("daemon-startup.log"), "raw stderr").unwrap();
        let out = dir.path().join("support.txt");

        write_export(dir.path(), &out).unwrap();

        let text = std::fs::read_to_string(out).unwrap();
        assert!(text.starts_with("CopyPaste "));
        assert!(text.contains("== app ==\n100 INFO copypaste::ui: opened <path>"));
        assert!(!text.contains("alice"));
        assert!(!text.contains("raw stderr"));
        assert_eq!(text.contains("peer unreachable"), INCLUDES_DAEMON);
    }
}
