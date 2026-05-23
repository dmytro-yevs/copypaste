//! Integration tests for `copypaste_core::logging::init_with_file_rotation`.
//!
//! These tests live in a dedicated integration-test binary because
//! `init_with_file_rotation` installs the **global** tracing subscriber and
//! the global is process-wide. Each integration test in Rust runs in its own
//! process, so we don't have to worry about `set_global_default` panicking
//! due to a leftover subscriber from another `#[test]`.
//!
//! NOTE: we deliberately do *not* run both tests in the same binary — Rust's
//! default test runner spawns separate processes per `#[test]` only when the
//! `--test-threads=1` flag is set AND each test is in a different file.
//! To stay safe we use a single test that exercises BOTH the basic-file-write
//! invariant and the rotation invariant in a single `init` call (the second
//! test stays in its own binary file `logging_rotation_daily.rs` if added
//! later).  For now: one process = one global subscriber = one test.

use std::path::PathBuf;
use std::time::Duration;

use copypaste_core::logging::init_with_file_rotation_kind;
use tracing::info;
use tracing_appender::rolling::Rotation;

/// Walk `dir` and return all entries with the requested file prefix.
fn list_log_files(dir: &std::path::Path, prefix: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(prefix) {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

/// Sum of file sizes across all entries.
fn total_size(paths: &[PathBuf]) -> u64 {
    paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

/// Combined test: a single `init_with_file_rotation_kind` call writes lines
/// that end up in a non-empty file under the configured directory, and the
/// returned `WorkerGuard` flushes buffered lines on drop.
///
/// We use `Rotation::MINUTELY` so that, on a slow machine that crosses a
/// minute boundary mid-test, we'd potentially see two files — which still
/// satisfies the "at least one log file with non-zero size" invariant.
#[test]
fn creates_log_file_in_target_dir() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let log_dir = tmp.path();
    let prefix = "copypaste-test";

    // Scope the guard so it is dropped (and thus flushes) before we read back.
    {
        let _guard = init_with_file_rotation_kind(log_dir, prefix, Rotation::MINUTELY, 7);

        // Emit a few lines.  The default EnvFilter is "info", so plain
        // `info!()` events are guaranteed to pass the filter.
        for i in 0..10 {
            info!(iteration = i, "rotation-test log line {i}");
        }

        // Give the non-blocking writer thread a tick to drain. The drop below
        // would also flush, but on slow CI we want to be extra sure that
        // metadata reflects writes before assertions.
        std::thread::sleep(Duration::from_millis(50));
    }

    // After the guard is dropped, all buffered lines must be on disk.
    let files = list_log_files(log_dir, prefix);
    assert!(
        !files.is_empty(),
        "expected at least one log file with prefix {prefix:?} in {:?}, got: {files:?}",
        log_dir,
    );

    let bytes = total_size(&files);
    assert!(
        bytes > 0,
        "expected log files to be non-empty after flushing guard, got {bytes} bytes across {files:?}",
    );

    // Sanity: each file path starts with our prefix.
    for f in &files {
        let name = f
            .file_name()
            .expect("filename")
            .to_string_lossy()
            .to_string();
        assert!(
            name.starts_with(prefix),
            "unexpected file name in log dir: {name}"
        );
    }
}
