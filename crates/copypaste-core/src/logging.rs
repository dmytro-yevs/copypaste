//! Logging helpers for CopyPaste library code.
//!
//! This module intentionally owns **only** test-time helpers.  The
//! production subscriber (`tracing-subscriber`, `tracing-appender`) is
//! initialized by each binary crate at its own entry point:
//!
//! * **copypaste-daemon** — `copypaste_daemon::logging::init()` (daily file
//!   rotation + stderr; see `crates/copypaste-daemon/src/logging.rs`).
//! * **copypaste-cli** — uses `tracing-subscriber` directly in `main()`.
//! * **copypaste-relay** — uses `tracing-subscriber` directly in `main()`.
//!
//! Keeping subscriber init out of the library prevents:
//! * Double-install panics when the library is embedded in a binary that
//!   has already set a global default.
//! * Unnecessary `tracing-subscriber` / `tracing-appender` bloat in the
//!   Android `.so` (UniFFI cdylib).
//!
//! The `tracing` facade crate (macros only, zero runtime overhead when no
//! subscriber is installed) remains a production dependency.

/// Environment variable selecting the log output format.  Accepts `"json"`
/// (case-insensitive) to enable structured JSON output.  Any other value (or
/// the variable being unset) yields the default compact text format.
pub const FORMAT_ENV: &str = "COPYPASTE_LOG_FORMAT";

/// Default directive applied when `RUST_LOG` is unset or unparsable.
pub const DEFAULT_FILTER: &str = "info";

/// Install a tracing subscriber for tests.
///
/// Uses a compact fmt layer wired to the test writer.  Safe to call from many
/// `#[test]` functions in the same process — the first call installs the
/// subscriber; subsequent calls silently no-op.
///
/// This function is **only available in test builds** (`cfg(test)` or when
/// `tracing-subscriber` is a direct dependency of the crate under test).
#[cfg(test)]
pub fn init_test() {
    use tracing_subscriber::{fmt::format::FmtSpan, prelude::*, EnvFilter, Registry};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let layer = tracing_subscriber::fmt::layer()
        .with_test_writer()
        .with_target(false)
        .with_span_events(FmtSpan::NONE);
    // Ignore the error — another test in this process may already have
    // installed the global subscriber.
    let _ = Registry::default().with(filter).with(layer).try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing::info;
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::{
        fmt::{self},
        prelude::*,
        EnvFilter, Registry,
    };

    fn build_env_filter() -> EnvFilter {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
    }

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
        let prev = std::env::var("RUST_LOG").ok();
        // SAFETY: tests in this module are single-threaded with respect to
        // this env var (we don't spawn threads here) and we restore below.
        std::env::set_var("RUST_LOG", "copypaste_core=debug");
        let filter = build_env_filter();
        let rendered = format!("{filter}");
        assert!(
            rendered.contains("copypaste_core") && rendered.contains("debug"),
            "expected filter to honor RUST_LOG, got: {rendered}"
        );
        match prev {
            Some(v) => std::env::set_var("RUST_LOG", v),
            None => std::env::remove_var("RUST_LOG"),
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
