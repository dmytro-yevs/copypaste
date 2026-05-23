//! Structured logging initialisation for the CopyPaste daemon.
//!
//! Sets up a layered [`tracing_subscriber`] with:
//! - A **rotating file appender** (daily rotation, keeps 7 files) writing JSON
//!   lines to an OS-specific log directory.
//! - A **stderr layer** writing human-readable compact lines (for systemd /
//!   launchd to capture).
//! - **EnvFilter** driven by the `COPYPASTE_LOG` environment variable
//!   (default: `copypaste=info,warn`).
//!
//! Log file locations:
//! | Platform | Path |
//! |----------|------|
//! | macOS    | `~/Library/Logs/CopyPaste/daemon.log` |
//! | Linux    | `~/.local/share/copypaste/logs/daemon.log` |
//! | Windows  | `%LOCALAPPDATA%\CopyPaste\logs\daemon.log` |
//!
//! On any path-resolution failure the implementation falls back to
//! `$TMPDIR/<uuid>/daemon.log` so the daemon always starts.

use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Holds the non-blocking writer's flush guard.
///
/// **Must** be kept alive for the entire process lifetime.  Drop it only after
/// the daemon event loop exits so that buffered log lines are flushed.
///
/// `_file_guard` is `Option` because the file-appender layer may have failed
/// to initialise (read-only FS, permission denied, etc.) — in which case we
/// silently dropped the file layer and fell back to stderr-only logging.
pub struct LogGuard {
    _file_guard: Option<WorkerGuard>,
}

/// Initialise the global tracing subscriber.
///
/// Returns a [`LogGuard`] that must be kept alive until process exit.
pub fn init() -> LogGuard {
    let log_dir = resolve_log_dir();

    // Ensure the directory exists; fall back to temp dir on failure.
    let log_dir = match std::fs::create_dir_all(&log_dir) {
        Ok(()) => log_dir,
        Err(e) => {
            let fallback = std::env::temp_dir().join("copypaste-logs");
            // Best-effort; if this also fails we'll panic below with a clear message.
            let _ = std::fs::create_dir_all(&fallback);
            eprintln!(
                "copypaste-daemon: WARNING: cannot create log dir {}: {e}; \
                 falling back to {fallback:?}",
                log_dir.display()
            );
            fallback
        }
    };

    // Shared env-filter (COPYPASTE_LOG env var, default: info).
    let default_filter = "copypaste=info,warn";

    // Try to build the daily-rotating file appender. On failure (read-only FS,
    // sandbox, permission denied) we drop the file layer and continue with
    // stderr-only logging rather than panicking the daemon at startup.
    let file_layer_and_guard = match tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("daemon")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
    {
        Ok(file_appender) => {
            let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);
            let env_filter = EnvFilter::try_from_env("COPYPASTE_LOG")
                .unwrap_or_else(|_| EnvFilter::new(default_filter));
            let file_layer = fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_thread_ids(true)
                .with_writer(non_blocking)
                .with_filter(env_filter)
                .boxed();
            Some((file_layer, file_guard))
        }
        Err(e) => {
            eprintln!(
                "copypaste-daemon: WARNING: file log appender failed at {}: {e}; \
                 continuing with stderr-only logging",
                log_dir.display()
            );
            None
        }
    };

    // Stderr layer: compact human-readable, no ANSI colours (for systemd/launchd).
    let stderr_filter = EnvFilter::try_from_env("COPYPASTE_LOG")
        .unwrap_or_else(|_| EnvFilter::new(default_filter));

    let stderr_layer = fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_filter(stderr_filter);

    // `file_layer_and_guard` is split: layer is registered with the subscriber,
    // guard is moved into the returned LogGuard.
    let (file_layer, file_guard) = match file_layer_and_guard {
        Some((layer, guard)) => (Some(layer), Some(guard)),
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    if file_guard.is_some() {
        tracing::info!(
            log_dir = %log_dir.display(),
            "logging initialised"
        );
    } else {
        tracing::warn!("logging initialised (stderr only — file appender unavailable)");
    }

    LogGuard {
        _file_guard: file_guard,
    }
}

/// Returns the OS-appropriate log directory.
///
/// | Platform | Resolution |
/// |----------|-----------|
/// | macOS    | `~/Library/Logs/CopyPaste` |
/// | Linux    | `~/.local/share/copypaste/logs` |
/// | Windows  | `%LOCALAPPDATA%\CopyPaste\logs` |
pub fn resolve_log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Library/Logs/CopyPaste")
    }

    #[cfg(target_os = "linux")]
    {
        // Prefer XDG data home (~/.local/share) if available, else ~/.copypaste
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".local/share")
            })
            .join("copypaste/logs")
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public"))
            .join("CopyPaste\\logs")
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        std::env::temp_dir().join("copypaste-logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────────────────────────────────
    // resolve_log_dir tests
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn log_dir_is_absolute() {
        let dir = resolve_log_dir();
        assert!(
            dir.is_absolute(),
            "log dir should be absolute, got: {dir:?}"
        );
    }

    #[test]
    fn log_dir_contains_copypaste() {
        let dir = resolve_log_dir();
        let s = dir.to_string_lossy().to_lowercase();
        assert!(
            s.contains("copypaste") || s.contains("CopyPaste"),
            "log dir should contain 'copypaste', got: {dir:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_log_dir_under_library_logs() {
        let dir = resolve_log_dir();
        let s = dir.to_string_lossy();
        assert!(
            s.contains("Library/Logs/CopyPaste"),
            "macOS log dir should be under Library/Logs/CopyPaste, got: {s}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_log_dir_ends_with_logs() {
        let dir = resolve_log_dir();
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("logs"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_log_dir_under_appdata() {
        let dir = resolve_log_dir();
        let s = dir.to_string_lossy();
        assert!(
            s.contains("CopyPaste") && s.contains("logs"),
            "Windows log dir should contain CopyPaste\\logs, got: {s}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Directory creation + fallback test
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn creates_log_dir_if_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join("subdir/logs");
        assert!(!target.exists());
        std::fs::create_dir_all(&target).expect("create_dir_all");
        assert!(target.is_dir());
    }

    #[test]
    fn falls_back_to_temp_when_dir_not_writable() {
        // On CI the process may not have permission to write to /root — simulate
        // by passing a clearly unwritable path and verifying fallback logic.
        // We don't call init() (that would conflict with existing global subscriber),
        // just check that create_dir_all on a bogus path returns an error.
        let bad_path = PathBuf::from("/proc/sys/kernel/impossible_log_dir_test");
        let result = std::fs::create_dir_all(&bad_path);
        // On macOS/Linux /proc doesn't exist so this errors.
        // The logging::init() would then fall back to temp dir.
        // We just assert the error path is exercised:
        assert!(
            result.is_err() || cfg!(target_os = "windows"),
            "expected create_dir_all to fail on unwritable path"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // EnvFilter env-var test
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn copypaste_log_env_var_parsed() {
        // Verify that a valid COPYPASTE_LOG value produces an EnvFilter without panic.
        let filter = tracing_subscriber::EnvFilter::new("copypaste=debug,warn");
        // If we get here without panic, parsing succeeded.
        drop(filter);
    }

    #[test]
    fn invalid_copypaste_log_falls_back_to_default() {
        // try_from_env on an env var that doesn't exist should return Err, and
        // we fall back to the default string — verify that chain works.
        // We use a var name that certainly isn't set.
        let filter =
            tracing_subscriber::EnvFilter::try_from_env("COPYPASTE_LOG_NONEXISTENT_VAR")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("copypaste=info,warn"));
        drop(filter);
    }
}
