//! Beta-bonus: export/import roundtrip + dedup integration tests.
//!
//! These tests drive the `copypaste` binary against a mock daemon backed by a
//! tempdir-scoped `UnixListener`. The socket path is injected via the
//! `COPYPASTE_SOCKET` env var (see `src/paths.rs`).
//!
//! Coverage matrix:
//! - `export_json_writes_valid_array_of_history_items`  — happy path, observable today
//! - `export_to_existing_file_refuses_without_force`    — DESIRED behavior; export.rs
//!   currently lacks a `--force` flag and clobbers unconditionally. Marked
//!   `#[ignore]` until the flag lands so this file documents the contract.
//! - `import_json_roundtrip_count_matches`              — daemon-dependent; the
//!   current `import` is a stub that only counts and prints. Asserts the count
//!   contract that the stub *does* honour (matches what export wrote).
//! - `import_invalid_json_returns_clear_error`          — happy path, observable today
//! - `import_partial_failure_continues_on_skip_flag`    — DESIRED; no `--skip`
//!   flag exists. `#[ignore]` until implemented.
//! - `import_dedupes_against_existing_db`               — DESIRED; needs a real
//!   `pin_item`/`store_item` round-trip through the daemon and content_hash
//!   dedup, neither of which the CLI stub exercises. `#[ignore]`.
//!
//! The mock daemon mirrors the protocol used in `src/ipc.rs::tests::mock_server`:
//! read one request line, write one JSON response line. Multi-call scenarios
//! re-bind a fresh listener per test to avoid cross-test races.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the freshly-compiled `copypaste` binary, courtesy of Cargo.
fn copypaste_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_copypaste"))
}

/// Spawn a one-shot mock daemon that accepts a single request, ignores its
/// content, and writes `response_json` followed by `\n`. The thread terminates
/// after the single round-trip — sufficient for the CLI's one-request-per-run
/// model.
///
/// Returns once the listener is bound, so callers don't need a sleep.
fn spawn_mock_daemon(socket_path: &Path, response_json: &'static str) {
    let listener = UnixListener::bind(socket_path).expect("bind mock daemon socket");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Drain the request line (we don't validate it — the CLI's wire
            // format is covered by ipc.rs unit tests).
            let mut buf = String::new();
            let mut reader = BufReader::new(&stream);
            let _ = reader.read_line(&mut buf);
            let _ = stream.write_all(response_json.as_bytes());
            let _ = stream.write_all(b"\n");
        }
    });
}

/// Build a canned `list` response containing `n` synthetic items. The shape
/// mirrors what `commands::list` returns from the daemon: a `{"items": [...]}`
/// object wrapped in the standard envelope.
///
/// We hard-code 3 items rather than parameterise, because the &'static lifetime
/// of `spawn_mock_daemon`'s payload would otherwise force an awkward leak.
fn canned_list_response_3_items() -> &'static str {
    // 3 items, each with a stable content_hash so the (future) dedup test
    // can reuse this fixture. Plain-text content keeps the JSON readable.
    //
    // MUST be single-line: the CLI reads daemon responses via `BufReader::read_line`,
    // which would otherwise truncate the payload at the first embedded `\n`.
    r#"{"id":"1","ok":true,"data":{"items":[{"id":"00000000-0000-0000-0000-000000000001","content":"alpha","content_hash":"hash-a","timestamp":1000,"kind":"text"},{"id":"00000000-0000-0000-0000-000000000002","content":"beta","content_hash":"hash-b","timestamp":2000,"kind":"text"},{"id":"00000000-0000-0000-0000-000000000003","content":"gamma","content_hash":"hash-c","timestamp":3000,"kind":"text"}]}}"#
}

/// Run `copypaste <args>` against the given socket path. Returns
/// (success, stdout, stderr) — same shape as the completions.rs harness so the
/// two test files stay readable side by side.
fn run_cli(socket: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(copypaste_bin())
        .env("COPYPASTE_SOCKET", socket)
        .args(args)
        .output()
        .expect("spawn copypaste binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ---------------------------------------------------------------------------
// Export tests
// ---------------------------------------------------------------------------

#[test]
fn export_json_writes_valid_array_of_history_items() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let out_file = dir.path().join("export.json");

    spawn_mock_daemon(&sock, canned_list_response_3_items());
    // Tiny delay only to win the race in CI containers where thread::spawn
    // can lag the listener.accept() loop by a few ms. The UnixListener is
    // already bound (bind() above is synchronous), so this is paranoia.
    thread::sleep(Duration::from_millis(20));

    let (ok, _stdout, stderr) = run_cli(
        &sock,
        &["export", "--output", out_file.to_str().unwrap()],
    );
    assert!(ok, "export failed: {stderr}");
    assert!(out_file.exists(), "export did not create output file");

    let written = std::fs::read_to_string(&out_file).expect("read export output");
    let parsed: serde_json::Value =
        serde_json::from_str(&written).expect("export output must be valid JSON");

    // Contract: export writes the daemon's `data` payload — a `{"items": [...]}`
    // object. The items field MUST be an array. We don't pin the exact count
    // to the canned 3 because future export.rs revisions may wrap or filter,
    // but the array-of-objects shape is the load-bearing invariant.
    let items = parsed
        .get("items")
        .and_then(|v| v.as_array())
        .expect("export JSON must contain an `items` array");
    assert!(
        !items.is_empty(),
        "export with non-empty daemon response must write a non-empty array"
    );
    for (i, item) in items.iter().enumerate() {
        assert!(
            item.get("id").is_some(),
            "item[{i}] missing `id` field: {item}"
        );
        assert!(
            item.get("content").is_some(),
            "item[{i}] missing `content` field: {item}"
        );
    }
}

/// DESIRED contract: exporting over an existing file must refuse unless
/// `--force` is passed. Current `export.rs` calls `std::fs::write`
/// unconditionally, so this test is `#[ignore]`d until the flag is wired up.
/// Flip the `#[ignore]` once `--force` lands and the test will guard against
/// accidental data loss regressions.
#[test]
#[ignore = "export.rs lacks --force flag; std::fs::write clobbers unconditionally (see import.rs:14 comment about Phase 4)"]
fn export_to_existing_file_refuses_without_force() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let out_file = dir.path().join("existing.json");
    std::fs::write(&out_file, r#"{"sentinel": "do not overwrite"}"#).unwrap();

    spawn_mock_daemon(&sock, canned_list_response_3_items());
    thread::sleep(Duration::from_millis(20));

    let (ok, _stdout, stderr) = run_cli(
        &sock,
        &["export", "--output", out_file.to_str().unwrap()],
    );

    assert!(!ok, "export over existing file must fail without --force");
    assert!(
        stderr.contains("exists") || stderr.contains("--force"),
        "stderr should mention the existing-file conflict, got: {stderr}"
    );

    // Sentinel must survive — the original file content is untouched.
    let after = std::fs::read_to_string(&out_file).unwrap();
    assert!(after.contains("sentinel"), "existing file was overwritten");
}

// ---------------------------------------------------------------------------
// Import tests
// ---------------------------------------------------------------------------

/// The current `import.rs` is a stub: it parses the file, counts items, and
/// prints `found N items in <file>`. We assert that contract end-to-end —
/// when desperate import → daemon write lands, this test still passes (the
/// count line should remain even after real insertion is added) and the
/// `#[ignore]`d roundtrip tests below take over the deeper contract.
#[test]
fn import_json_roundtrip_count_matches() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let export_file = dir.path().join("dump.json");

    // 1) Export via mock daemon.
    spawn_mock_daemon(&sock, canned_list_response_3_items());
    thread::sleep(Duration::from_millis(20));
    let (ok_e, _, stderr_e) = run_cli(
        &sock,
        &["export", "--output", export_file.to_str().unwrap()],
    );
    assert!(ok_e, "export step failed: {stderr_e}");

    // 2) Re-import the same file. Import is a stub that does not contact the
    //    daemon (see import.rs); no second mock is needed. When import gains
    //    daemon support, spawn a second mock_daemon here that accepts N store
    //    requests and returns `{"ok": true}` for each.
    let (ok_i, stdout_i, stderr_i) = run_cli(
        &sock,
        &["import", export_file.to_str().unwrap()],
    );
    assert!(ok_i, "import step failed: {stderr_i}");

    // The stub prints `found N items in <file>` — match the 3 items from the
    // canned response. This is the strongest assertion we can make today
    // without daemon-side write support.
    assert!(
        stdout_i.contains("found 3 items"),
        "import did not report the expected count; stdout={stdout_i}"
    );
}

#[test]
fn import_invalid_json_returns_clear_error() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let bad_file = dir.path().join("not-json.json");
    std::fs::write(&bad_file, "this is { not valid json").unwrap();

    // No mock daemon: import.rs parses the file before any IPC, so a missing
    // socket would only matter if parsing succeeded. We deliberately point at
    // a nonexistent socket to prove parse-time errors short-circuit cleanly.
    let (ok, _stdout, stderr) = run_cli(
        &sock,
        &["import", bad_file.to_str().unwrap()],
    );
    assert!(!ok, "import of malformed JSON must exit non-zero");
    // The CLI's main wraps errors as `copypaste: <err>`. The underlying parse
    // error comes from serde_json and contains `expected` / `key` / `line`.
    // We assert on the `copypaste:` prefix (stable) plus *some* JSON-shaped
    // signal, to keep the test resistant to serde_json wording drift.
    assert!(
        stderr.contains("copypaste:"),
        "stderr must carry the `copypaste:` error prefix, got: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("json")
            || stderr.contains("expected")
            || stderr.contains("invalid"),
        "stderr should describe the JSON parse failure, got: {stderr}"
    );
}

/// DESIRED: with `--skip` (or `--continue-on-error`), one malformed row in an
/// otherwise-valid file should be reported and skipped while the rest are
/// imported. Current import.rs has no row-level iteration — it either fully
/// parses the wrapping object or bails. `#[ignore]` until a `--skip` flag is
/// introduced and per-item iteration is added.
#[test]
#[ignore = "import.rs lacks --skip flag and per-row error handling (Phase 4 daemon-write feature)"]
fn import_partial_failure_continues_on_skip_flag() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let mixed_file = dir.path().join("mixed.json");

    // Two valid items + one row missing required field `content`. Once
    // import.rs validates per-row, the middle entry should be skipped.
    let payload = r#"{"items":[
        {"id":"a","content":"good","content_hash":"h-a","timestamp":1,"kind":"text"},
        {"id":"b","content_hash":"h-b","timestamp":2,"kind":"text"},
        {"id":"c","content":"alsogood","content_hash":"h-c","timestamp":3,"kind":"text"}
    ]}"#;
    std::fs::write(&mixed_file, payload).unwrap();

    // Mock daemon that accepts every `store` request and returns success.
    // (When this test is un-ignored, replace the canned list response with a
    // multi-call mock that counts store invocations.)
    spawn_mock_daemon(&sock, r#"{"id":"1","ok":true,"data":{"stored":true}}"#);
    thread::sleep(Duration::from_millis(20));

    let (ok, stdout, stderr) = run_cli(
        &sock,
        &["import", mixed_file.to_str().unwrap(), "--skip"],
    );
    assert!(ok, "import with --skip must succeed despite bad rows: {stderr}");
    assert!(
        stdout.contains("imported 2") || stdout.contains("skipped 1"),
        "import must report skip/imported counts, got: {stdout}"
    );
}

/// DESIRED: re-importing the same file twice must NOT double-insert items
/// whose `content_hash` is already present in the database. The current stub
/// has no DB write path at all, so this is a forward-looking contract test.
/// Un-ignore when import.rs gains a content_hash-based dedup check (either
/// CLI-side `dedupe_seen_hashes` or daemon-side `INSERT OR IGNORE`).
#[test]
#[ignore = "import.rs does not write to daemon yet; content_hash dedup is a Phase 4+ feature"]
fn import_dedupes_against_existing_db() {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("daemon.sock");
    let dump = dir.path().join("dump.json");

    // First export+import populates the DB. Second import of the same file
    // should be a no-op (every content_hash already known).
    spawn_mock_daemon(&sock, canned_list_response_3_items());
    thread::sleep(Duration::from_millis(20));
    let (ok_e, _, _) = run_cli(&sock, &["export", "--output", dump.to_str().unwrap()]);
    assert!(ok_e);

    // Pass 1: 3 inserts.
    let (ok1, stdout1, _) = run_cli(&sock, &["import", dump.to_str().unwrap()]);
    assert!(ok1);
    assert!(stdout1.contains("imported 3") || stdout1.contains("3 items"));

    // Pass 2: 0 inserts because content_hash matches. The CLI must report
    // either `skipped 3 (duplicates)` or `imported 0`.
    let (ok2, stdout2, _) = run_cli(&sock, &["import", dump.to_str().unwrap()]);
    assert!(ok2);
    assert!(
        stdout2.contains("skipped 3")
            || stdout2.contains("imported 0")
            || stdout2.contains("duplicates"),
        "second import must report dedup, got: {stdout2}"
    );
}
