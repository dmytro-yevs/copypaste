//! Tracing initialization helpers for CopyPaste.
//!
//! This module provides two entry points:
//!
//! * [`init_global`] — installs a process-wide [`tracing_subscriber`] with an
//!   [`EnvFilter`] derived from the `RUST_LOG` environment variable
//!   (defaulting to `"info"`). The output format defaults to a compact
//!   human-readable layer; setting `COPYPASTE_LOG_FORMAT=json` switches to
//!   structured JSON suitable for log aggregators.
//!
//! * [`init_test`] — best-effort initializer for unit/integration tests. It
//!   uses [`try_init`](tracing_subscriber::util::SubscriberInitExt::try_init)
//!   and ignores the error when a subscriber is already installed, so it is
//!   safe to call from many tests in the same process.
//!
//! Both helpers are intentionally side-effect-only: they install the global
//! default subscriber and return. Consumers (daemon, cli, ui) wire them in
//! at their respective entry points; this crate only owns the helper itself.

use std::env;
use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder as RollingBuilder, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter, Layer, Registry,
};

/// Environment variable selecting the log output format. Accepts `"json"`
/// (case-insensitive) to enable structured JSON output. Any other value (or
/// the variable being unset) yields the default compact text format.
pub const FORMAT_ENV: &str = "COPYPASTE_LOG_FORMAT";

/// Default directive applied when `RUST_LOG` is unset or unparsable.
pub const DEFAULT_FILTER: &str = "info";

/// Install a process-wide tracing subscriber.
///
/// * Filter: built from `RUST_LOG`, falling back to [`DEFAULT_FILTER`].
/// * Format: JSON when `COPYPASTE_LOG_FORMAT=json`, compact text otherwise.
///
/// Returns `Ok(())` on success, or `Err` if a global subscriber was already
/// installed (e.g. by an earlier `init_global` call or by a test harness).
/// Errors are intentionally non-fatal — callers may log and continue.
pub fn init_global() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = build_env_filter();

    if want_json() {
        let layer = fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)
            .with_span_events(FmtSpan::NONE);
        Registry::default().with(filter).with(layer).try_init()?;
    } else {
        let layer = fmt::layer()
            .compact()
            .with_target(true)
            .with_span_events(FmtSpan::NONE);
        Registry::default().with(filter).with(layer).try_init()?;
    }

    Ok(())
}

/// Install a process-wide tracing subscriber that writes to BOTH a daily-rotating
/// log file and stdout (compact human-readable).
///
/// File rotation policy:
/// * Rotation period: daily (one file per UTC day).
/// * Retention: at most **7 files** kept on disk — older files are removed by
///   `tracing-appender` as new ones are created.
/// * File name pattern: `{prefix}.YYYY-MM-DD.log` inside `log_dir`.
///
/// Both sinks share the same [`EnvFilter`] derived from `RUST_LOG` (default
/// [`DEFAULT_FILTER`]). The stdout sink keeps interactive (foreground) daemon
/// output visible; the file sink is for post-mortem investigation.
///
/// The returned [`WorkerGuard`] **must** be kept alive until process exit so
/// that buffered log lines are flushed before the non-blocking writer shuts
/// down. Dropping it early may truncate the tail of the log.
///
/// On failure to build the file appender (read-only fs, permission denied, …)
/// this function falls back to **stdout-only** logging and prints a warning to
/// stderr — the process still receives a working subscriber.
///
/// Panics if a global subscriber was already installed by something else
/// (intentional — the daemon binary owns the global subscriber for its
/// process and a duplicate install is a programmer error).
pub fn init_with_file_rotation(log_dir: &Path, prefix: &str) -> WorkerGuard {
    init_with_file_rotation_kind(log_dir, prefix, Rotation::DAILY, 7)
}

/// Variant of [`init_with_file_rotation`] that exposes the rotation kind and
/// retention count for tests (so we can use HOURLY/MINUTELY rotation without
/// waiting a full day). Production callers should prefer the public alias above.
#[doc(hidden)]
pub fn init_with_file_rotation_kind(
    log_dir: &Path,
    prefix: &str,
    rotation: Rotation,
    max_log_files: usize,
) -> WorkerGuard {
    // Best-effort directory creation; non-fatal — file builder will surface a
    // clearer error if the path really is unusable.
    let _ = std::fs::create_dir_all(log_dir);

    let file_appender_result = RollingBuilder::new()
        .rotation(rotation)
        .filename_prefix(prefix)
        .filename_suffix("log")
        .max_log_files(max_log_files)
        .build(log_dir);

    // Build stdout layer once. We construct two boxed Option<Layer> shapes
    // so the final subscriber type is identical on both branches — that
    // keeps the `Registry::default().with(...).with(...).init()` call type
    // monomorphic and within tracing-subscriber's `SubscriberInitExt`
    // implementation surface.
    let stdout_layer = fmt::layer()
        .compact()
        .with_target(true)
        .with_ansi(false)
        .with_writer(std::io::stdout)
        .with_span_events(FmtSpan::NONE)
        .with_filter(build_env_filter())
        .boxed();

    let (file_layer_opt, guard): (
        Option<Box<dyn Layer<Registry> + Send + Sync>>,
        WorkerGuard,
    ) = match file_appender_result {
        Ok(file_appender) => {
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let file_layer = fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(false)
                .with_writer(non_blocking)
                .with_span_events(FmtSpan::CLOSE)
                .with_filter(build_env_filter())
                .boxed();
            (Some(file_layer), guard)
        }
        Err(e) => {
            eprintln!(
                "copypaste: WARNING: file log appender failed at {}: {e}; \
                 continuing with stdout-only logging",
                log_dir.display()
            );
            // Return a guard for a discarded non-blocking sink so the
            // caller's API stays uniform.
            let (_sink, guard) = tracing_appender::non_blocking(std::io::sink());
            (None, guard)
        }
    };

    // `.with(Option<L>)` is equivalent to omitting the layer when None,
    // and identically nested when Some — keeping the subscriber type stable.
    Registry::default()
        .with(file_layer_opt)
        .with(stdout_layer)
        .init();

    guard
}

/// Install a tracing subscriber for tests.
///
/// Uses the same filter/format rules as [`init_global`] but never panics or
/// returns an error when a subscriber is already installed — this lets many
/// `#[test]` functions call it without coordination.
pub fn init_test() {
    let filter = build_env_filter();
    let layer = fmt::layer()
        .with_test_writer()
        .with_target(false)
        .with_span_events(FmtSpan::NONE);
    // Ignore the error — another test in this process may already have
    // installed the global subscriber.
    let _ = Registry::default().with(filter).with(layer).try_init();
}

fn build_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
}

fn want_json() -> bool {
    matches!(
        env::var(FORMAT_ENV).ok().as_deref().map(str::to_ascii_lowercase),
        Some(ref v) if v == "json"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::info;
    use tracing_subscriber::fmt::MakeWriter;

    /// Shared buffer writer used to capture formatted log lines during tests.
    #[derive(Clone, Default)]
    struct BufWriter(Arc<Mutex<Vec<u8>>>);

    impl BufWriter {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap_or_default()
        }
    }

    impl std::io::Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for BufWriter {
        type Writer = BufWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn init_test_is_idempotent() {
        // Calling init_test multiple times must not panic, even though only
        // the first call actually installs a global subscriber.
        init_test();
        init_test();
        init_test();
    }

    #[test]
    fn env_filter_respects_rust_log_var() {
        // Save & override RUST_LOG for this test, then restore. The helper
        // must produce a filter that contains the supplied directive.
        let prev = env::var("RUST_LOG").ok();
        // SAFETY: tests in this module are single-threaded with respect to
        // this env var (we don't spawn threads here) and we restore below.
        env::set_var("RUST_LOG", "copypaste_core=debug");
        let filter = build_env_filter();
        let rendered = format!("{filter}");
        assert!(
            rendered.contains("copypaste_core") && rendered.contains("debug"),
            "expected filter to honor RUST_LOG, got: {rendered}"
        );
        match prev {
            Some(v) => env::set_var("RUST_LOG", v),
            None => env::remove_var("RUST_LOG"),
        }
    }

    #[test]
    fn json_format_emits_valid_json_event() {
        // Build a JSON fmt layer wired to a capture buffer, install it as a
        // local subscriber for the scope of this test, emit one event, then
        // parse the captured bytes as JSON and assert the expected fields.
        let buf = BufWriter::default();
        let layer = fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(buf.clone());
        let filter = EnvFilter::new("info");
        let subscriber = Registry::default().with(filter).with(layer);

        tracing::subscriber::with_default(subscriber, || {
            info!(answer = 42, "hello-json");
        });

        let output = buf.contents();
        assert!(!output.is_empty(), "no log output captured");
        // The fmt layer writes one JSON object per line.
        let first_line = output.lines().next().expect("at least one line");
        let parsed: serde_json::Value = serde_json::from_str(first_line)
            .unwrap_or_else(|e| panic!("invalid JSON: {e}; line was: {first_line}"));
        assert_eq!(parsed["level"], "INFO");
        assert_eq!(parsed["fields"]["message"], "hello-json");
        assert_eq!(parsed["fields"]["answer"], 42);
    }
}
