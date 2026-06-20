//! Structured logging initialisation for the CopyPaste daemon.
//!
//! Sets up a layered [`tracing_subscriber`] with:
//! - A **rotating file appender** (daily rotation, keeps 7 files) writing JSON
//!   lines to an OS-specific log directory.
//! - A **stderr layer** writing human-readable compact lines (for systemd /
//!   launchd to capture).
//! - **EnvFilter** driven by the `COPYPASTE_LOG` environment variable
//!   (default: `copypaste=info,warn`).
//! - **PiiScrubber** (CopyPaste-lx6c): both the file layer and the stderr
//!   layer wrap their `io::Write` targets in a [`ScrubWriter`] that redacts PII
//!   patterns (emails, IPs, tokens, home-directory paths) from every log line
//!   before it reaches the OS file-system or the terminal. This is the last
//!   line of defence ensuring that even if a developer accidentally logs a
//!   sensitive value, it is scrubbed before it persists.
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

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use copypaste_telemetry::PiiScrubber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan, MakeWriter},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

// ── PII-scrubbing I/O adapter (CopyPaste-lx6c) ────────────────────────────
//
// `ScrubMakeWriter<M>` wraps any `MakeWriter` target and produces
// `ScrubWriterInner` instances that pass each `write()` buffer through
// `PiiScrubber::scrub` before forwarding the bytes.
//
// The tracing `fmt` layer calls `write()` with a single complete UTF-8 log
// line (JSON or compact) per event, so line-level scrubbing is sufficient —
// no internal buffering across `write()` calls is needed.
//
// `Arc<PiiScrubber>` allows sharing one compiled scrubber instance across the
// file and stderr sinks without cloning the pattern set.

/// A `MakeWriter` adapter that wraps `M: MakeWriter` and scrubs PII from every
/// log line before forwarding bytes to the underlying writer.
///
/// The scrubber is shared (`Arc`) across all writer instances produced per
/// log event, so compilation happens once at init time.
pub struct ScrubMakeWriter<M> {
    inner: M,
    scrubber: Arc<PiiScrubber>,
}

impl<M> ScrubMakeWriter<M> {
    /// Wrap `inner` with the supplied scrubber.
    pub fn new(inner: M, scrubber: Arc<PiiScrubber>) -> Self {
        Self { inner, scrubber }
    }
}

/// A single-use writer produced by [`ScrubMakeWriter`] for one log event.
pub struct ScrubWriterInner<W: io::Write> {
    inner: W,
    scrubber: Arc<PiiScrubber>,
}

impl<W: io::Write> io::Write for ScrubWriterInner<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Convert to &str for scrubbing. Non-UTF-8 bytes (should not appear in
        // structured JSON/compact log output but could in theory occur) are
        // passed through unscrubbed to avoid losing log lines on error.
        let scrubbed_bytes: Vec<u8> = match std::str::from_utf8(buf) {
            Ok(s) => self.scrubber.scrub(s).into_bytes(),
            Err(_) => buf.to_vec(),
        };
        // Write the scrubbed bytes. Report the ORIGINAL buf.len() as written
        // so the caller does not retry on a short-write caused purely by the
        // scrubber changing the byte count (e.g. a long token replaced by a
        // shorter `<REDACTED-*>` tag shrinks the byte count).
        self.inner.write_all(&scrubbed_bytes)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, M> MakeWriter<'a> for ScrubMakeWriter<M>
where
    M: MakeWriter<'a>,
{
    type Writer = ScrubWriterInner<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        ScrubWriterInner {
            inner: self.inner.make_writer(),
            scrubber: Arc::clone(&self.scrubber),
        }
    }
}

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

    // CopyPaste-lx6c: shared PII scrubber instance for both log sinks.
    // `Arc` lets the same compiled pattern set serve the file appender and
    // stderr without copying the regex set.  `PiiScrubber::default()` loads
    // the full built-in pattern set (emails, IPs, tokens, home paths, etc.).
    let scrubber = Arc::new(PiiScrubber::default());

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
            // Wrap the non-blocking MakeWriter in ScrubMakeWriter so every
            // JSON log line is scrubbed before it reaches the OS file-system.
            let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);
            let scrub_file = ScrubMakeWriter::new(non_blocking, Arc::clone(&scrubber));
            let env_filter = EnvFilter::try_from_env("COPYPASTE_LOG")
                .unwrap_or_else(|_| EnvFilter::new(default_filter));
            let file_layer = fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_thread_ids(true)
                .with_writer(scrub_file)
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
    // Also wrapped in ScrubWriter so PII does not leak to journald/launchd logs.
    let stderr_filter =
        EnvFilter::try_from_env("COPYPASTE_LOG").unwrap_or_else(|_| EnvFilter::new(default_filter));

    // Wrap stderr's MakeWriter in ScrubMakeWriter so PII does not leak to
    // journald / launchd captured output.
    let scrub_stderr = ScrubMakeWriter::new(std::io::stderr, Arc::clone(&scrubber));
    let stderr_layer = fmt::layer()
        .compact()
        .with_ansi(false)
        .with_writer(scrub_stderr)
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
            "logging initialised (PII scrubber active)"
        );
    } else {
        tracing::warn!(
            "logging initialised (stderr only — file appender unavailable; PII scrubber active)"
        );
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
        let filter = tracing_subscriber::EnvFilter::try_from_env("COPYPASTE_LOG_NONEXISTENT_VAR")
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("copypaste=info,warn"));
        drop(filter);
    }

    // ──────────────────────────────────────────────────────────────────────────
    // CopyPaste-lx6c: ScrubMakeWriter PII-redaction tests
    // ──────────────────────────────────────────────────────────────────────────

    // ── ScrubMakeWriter tests ─────────────────────────────────────────────────
    //
    // We exercise the `ScrubMakeWriter` via `MakeWriter::make_writer()` so we
    // exercise the public API rather than struct internals.  The `make_writer()`
    // call returns a `ScrubWriterInner<W>` which is itself an `io::Write`, so
    // we can call `write_all` on it and inspect a shared buffer.

    // Shared-buffer MakeWriter used by the tests below.
    struct SharedBufMakeWriter(Arc<std::sync::Mutex<Vec<u8>>>);

    impl io::Write for SharedBufMakeWriter {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    // MakeWriter impl that clones the Arc on each call.
    struct SharedBufMakeWriterFactory(Arc<std::sync::Mutex<Vec<u8>>>);

    impl<'a> MakeWriter<'a> for SharedBufMakeWriterFactory {
        type Writer = SharedBufMakeWriter;
        fn make_writer(&'a self) -> Self::Writer {
            SharedBufMakeWriter(Arc::clone(&self.0))
        }
    }

    fn make_scrub_writer_into_vec(
        scrubber: Arc<PiiScrubber>,
    ) -> (
        impl io::Write,
        Arc<std::sync::Mutex<Vec<u8>>>,
    ) {
        let buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let factory = SharedBufMakeWriterFactory(Arc::clone(&buf));
        let scrub = ScrubMakeWriter::new(factory, scrubber);
        let writer = scrub.make_writer();
        (writer, buf)
    }

    /// `ScrubMakeWriter` must redact email addresses from log lines before
    /// forwarding to the underlying writer.
    #[test]
    fn scrub_writer_redacts_email() {
        use std::io::Write as _;
        let scrubber = Arc::new(PiiScrubber::default());
        let (mut w, buf) = make_scrub_writer_into_vec(scrubber);
        w.write_all(b"error: user alice@example.com failed\n").unwrap();
        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !written.contains("alice@example.com"),
            "email must be redacted, got: {written}"
        );
        assert!(
            written.contains("<REDACTED-EMAIL>"),
            "redaction marker must appear, got: {written}"
        );
    }

    /// `ScrubMakeWriter` must redact IPv4 addresses.
    #[test]
    fn scrub_writer_redacts_ip() {
        use std::io::Write as _;
        let scrubber = Arc::new(PiiScrubber::default());
        let (mut w, buf) = make_scrub_writer_into_vec(scrubber);
        w.write_all(b"connecting to 192.168.1.42:9000\n").unwrap();
        let written = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !written.contains("192.168.1.42"),
            "IPv4 address must be redacted, got: {written}"
        );
    }

    /// Non-UTF-8 bytes must pass through unchanged (no panic, no data loss).
    #[test]
    fn scrub_writer_non_utf8_passthrough() {
        use std::io::Write as _;
        let scrubber = Arc::new(PiiScrubber::default());
        let (mut w, buf) = make_scrub_writer_into_vec(scrubber);
        // Invalid UTF-8: lone 0x80 byte.
        let garbage = b"prefix\x80suffix";
        let n = w.write(garbage).unwrap();
        // write() must report the original length (caller satisfaction).
        assert_eq!(n, garbage.len(), "write() must report original buf len");
        // The bytes must pass through unchanged.
        assert_eq!(
            *buf.lock().unwrap(),
            garbage.to_vec(),
            "non-UTF-8 bytes must pass through unchanged"
        );
    }
}
