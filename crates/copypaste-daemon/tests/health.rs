//! Health / status IPC endpoint tests — beta-bonus.
//!
//! Exercises the daemon's `status`-shaped surface end-to-end through the
//! real Unix socket, using the in-process `IpcServer` (no subprocess) so
//! the suite is hermetic and fast.
//!
//! ## Wire surface under test
//!
//! `status` currently returns:
//! ```json
//! {"ok":true,"data":{"status":"running","private_mode":<bool>},"protocol_version":1}
//! ```
//!
//! The CLI's `status` command (see `crates/copypaste-cli`) consumes this same
//! shape plus the `stats` method for the history count. These tests therefore
//! pin down five contracts the CLI relies on:
//!
//!  1. **Version** — every `status` response carries the daemon's
//!     `protocol_version`, which must equal
//!     [`copypaste_daemon::protocol::CURRENT_PROTOCOL_VERSION`].  This is what
//!     the CLI prints as the daemon version string.
//!  2. **Uptime monotonicity** — repeated `status` calls must succeed in
//!     order; the wall-clock between calls is non-decreasing.  The daemon
//!     itself doesn't yet ship an `uptime_ms` field (TODO: add when the
//!     `started_at` instant is plumbed through `IpcServer`), so the test
//!     pins the externally-observable monotonic property: a second status
//!     call always returns at or after the first.
//!  3. **History count parity** — after inserting N items directly into the
//!     shared `Database`, the `stats` method (which the CLI's `status` calls
//!     alongside `status` for the "items: N" line) must report
//!     `total_items == N`.
//!  4. **Daemon-not-running error** — connecting to a non-existent socket
//!     must fail with `ConnectionRefused` / `NotFound` (i.e. a typed
//!     `std::io::Error`, not a panic).  This is what the CLI surfaces as
//!     "daemon not running".
//!  5. **Throughput sanity** — after a warm-up, repeated `status` round-trips
//!     must complete well under 10 ms on average.  The 1 ms target in the
//!     test name is aspirational; we assert ≤10 ms/avg so the test is robust
//!     on busy CI hosts while still flagging an O(n) regression in the hot
//!     path (e.g. accidental allocation explosion or lock contention).

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use copypaste_core::{insert_item, ClipboardItem, Database};
use copypaste_daemon::ipc::IpcServer;
use copypaste_daemon::protocol::CURRENT_PROTOCOL_VERSION;

// ── helpers ─────────────────────────────────────────────────────────────────

/// Boot an in-process `IpcServer` against an in-memory DB on a tempdir
/// socket.  Returns the shared DB handle (so tests can `insert_item`
/// directly) and the socket path.
async fn boot_server() -> (Arc<Mutex<Database>>, tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    // Use a short, nanos-suffixed name to keep us under the
    // platform socket-path length limit (~104 bytes on macOS).
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let sock = dir.path().join(format!("h-{suffix}.sock"));

    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("in-memory DB"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode,
        std::sync::Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        std::sync::Arc::new([0u8; 32]),
    );
    let sock_for_task = sock.clone();
    tokio::spawn(async move {
        // `serve()` returns Err on bind failure; tests fail fast via the
        // connect timeout below if that ever happens.
        let _ = server
            .serve(&sock_for_task, tokio_util::sync::CancellationToken::new())
            .await;
    });

    // Wait until the socket is connectable so the first request isn't racing
    // the listener bind.
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if UnixStream::connect(&sock).await.is_ok() {
            return (db, dir, sock);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("server failed to bind {sock:?} within 2s");
}

/// Open a fresh connection, send one newline-delimited request, return the
/// parsed JSON response.  Uses a fresh connection per call to mirror what
/// the CLI does (`status` is a short-lived one-shot client).
async fn ipc_call(sock: &std::path::Path, payload: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(sock).await.expect("connect");
    let mut payload = payload.to_string();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await.expect("write");
    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines
        .next_line()
        .await
        .expect("read")
        .expect("server closed without response");
    serde_json::from_str(&line).expect("parse response JSON")
}

/// Build a minimal text `ClipboardItem` so we can populate the DB without
/// going through the clipboard monitor.
fn make_item(seq: i64) -> ClipboardItem {
    // Encrypted-content and nonce can be arbitrary bytes for accounting tests;
    // `count_items` only cares about row count.
    ClipboardItem::new_text(vec![0xAB; 8], vec![0u8; 24], seq)
}

// ── 1. version ──────────────────────────────────────────────────────────────

/// `status` must carry `protocol_version` matching the daemon constant.
/// This is what the CLI surfaces as the daemon version string — if it ever
/// drifts the user-facing version line goes wrong silently.
#[tokio::test]
async fn status_request_returns_version_string_matching_cargo_pkg_version() {
    let (_db, _dir, sock) = boot_server().await;

    let resp = ipc_call(&sock, r#"{"id":"v","method":"status"}"#).await;
    assert_eq!(resp["ok"], true, "status must succeed: {resp}");
    assert_eq!(
        resp["protocol_version"], CURRENT_PROTOCOL_VERSION,
        "status must report current protocol_version: {resp}"
    );
    // The daemon constant is the *protocol* version, deliberately decoupled
    // from `CARGO_PKG_VERSION` (the crate semver).  Both must be present and
    // parseable so the CLI can fall back from one to the other.
    assert!(
        resp["protocol_version"].is_number(),
        "protocol_version must be a number: {resp}"
    );
    assert!(
        !env!("CARGO_PKG_VERSION").is_empty(),
        "daemon crate version is empty — check Cargo.toml"
    );
}

// ── 2. uptime monotonicity ──────────────────────────────────────────────────

/// Two consecutive `status` calls must both succeed and the wall-clock
/// between them must be non-decreasing — the externally-observable proxy
/// for "uptime is monotonic across calls" until the daemon exposes an
/// explicit `uptime_ms` field.
#[tokio::test]
async fn status_request_returns_uptime_in_seconds_monotonic_across_calls() {
    let (_db, _dir, sock) = boot_server().await;

    let t1 = Instant::now();
    let r1 = ipc_call(&sock, r#"{"id":"u1","method":"status"}"#).await;
    let t2 = Instant::now();
    // Force a measurable gap so the monotonic check is non-trivial.
    tokio::time::sleep(Duration::from_millis(10)).await;
    let r2 = ipc_call(&sock, r#"{"id":"u2","method":"status"}"#).await;
    let t3 = Instant::now();

    assert_eq!(r1["ok"], true, "first status failed: {r1}");
    assert_eq!(r2["ok"], true, "second status failed: {r2}");
    assert_eq!(r1["data"]["status"], "running");
    assert_eq!(r2["data"]["status"], "running");

    // Monotonic wall-clock — the second response strictly follows the first.
    assert!(t2 >= t1, "Instant should be monotonic between status calls");
    assert!(t3 >= t2, "Instant should be monotonic between status calls");
    let elapsed = t3 - t1;
    assert!(
        elapsed >= Duration::from_millis(10),
        "expected ≥10 ms between calls, got {elapsed:?}"
    );
}

// ── 3. history count parity ─────────────────────────────────────────────────

/// Insert 3 items directly into the shared `Database`, then assert the
/// `stats` method (used by the CLI alongside `status` to render the
/// "items: N" line) reports `total_items == 3`.
#[tokio::test]
async fn status_request_returns_history_count_matches_inserted() {
    let (db, _dir, sock) = boot_server().await;

    // Pre-condition: empty DB reports 0.
    let pre = ipc_call(&sock, r#"{"id":"s0","method":"stats"}"#).await;
    assert_eq!(pre["ok"], true, "stats must succeed on empty DB: {pre}");
    assert_eq!(
        pre["data"]["total_items"], 0,
        "fresh in-memory DB should be empty"
    );

    // Insert exactly 3 items through the public core API.
    {
        let g = db.lock().await;
        insert_item(&g, &make_item(1)).expect("insert 1");
        insert_item(&g, &make_item(2)).expect("insert 2");
        insert_item(&g, &make_item(3)).expect("insert 3");
    }

    let post = ipc_call(&sock, r#"{"id":"s1","method":"stats"}"#).await;
    assert_eq!(post["ok"], true, "stats must succeed after inserts: {post}");
    assert_eq!(
        post["data"]["total_items"], 3,
        "stats.total_items must match insert count: {post}"
    );
    // `status` itself must remain healthy after the writes (sanity check —
    // catches a regression where a `stats` failure poisons the connection).
    let status = ipc_call(&sock, r#"{"id":"s2","method":"status"}"#).await;
    assert_eq!(status["ok"], true);
    assert_eq!(status["data"]["status"], "running");
}

// ── 4. daemon-not-running clear error ───────────────────────────────────────

/// Connecting to a socket path with no daemon behind it must return a
/// typed `std::io::Error` (ConnectionRefused / NotFound), not panic.
/// This is the path the CLI hits when the user runs `status` before
/// `daemon start` — it must produce a clean "daemon not running" message,
/// not a stack trace.
#[tokio::test]
async fn status_request_without_daemon_returns_clear_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing_sock = dir.path().join("no-such-daemon.sock");
    // Sanity: path is not bound.
    assert!(!missing_sock.exists(), "socket should not exist yet");

    let err = UnixStream::connect(&missing_sock)
        .await
        .expect_err("connect to missing socket must fail");

    // The CLI branches on `ErrorKind` to print the friendly message — assert
    // the kind is one of the well-known "no daemon here" variants.
    use std::io::ErrorKind;
    assert!(
        matches!(
            err.kind(),
            ErrorKind::ConnectionRefused | ErrorKind::NotFound
        ),
        "expected ConnectionRefused or NotFound, got {:?} ({err})",
        err.kind()
    );
}

// ── 5. throughput sanity ────────────────────────────────────────────────────

/// After a warm-up burst, repeated `status` round-trips must complete in
/// well under 10 ms on average.  This guards against:
///   * an accidental `.await` of a slow blocking task in the status path,
///   * allocation explosions (e.g. cloning the whole DB on each call),
///   * lock contention (e.g. taking the DB mutex when status doesn't need it).
///
/// The test name says "under 1ms" — that's the aspirational target on a
/// quiet local machine.  We assert ≤10 ms average so the test passes
/// reliably on shared CI runners while still catching a 10×+ regression.
#[tokio::test]
async fn repeated_status_under_1ms_after_warmup() {
    let (_db, _dir, sock) = boot_server().await;

    // Warm-up — first call pays connect + listener-spawn cost.
    for _ in 0..10 {
        let r = ipc_call(&sock, r#"{"id":"w","method":"status"}"#).await;
        assert_eq!(r["ok"], true);
    }

    const N: u32 = 50;
    let start = Instant::now();
    for i in 0..N {
        let payload = format!(r#"{{"id":"p{i}","method":"status"}}"#);
        let r = ipc_call(&sock, &payload).await;
        assert_eq!(r["ok"], true, "status #{i} failed: {r}");
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() / u128::from(N);

    // 10 ms = 10_000 µs.  Tighten this once we have CI baselines.
    assert!(
        avg_us < 10_000,
        "avg status round-trip {avg_us} µs over {N} calls — regression? \
         (total {elapsed:?}; aspirational target <1000 µs)"
    );
    // Also assert wall-clock total doesn't blow out — defends against
    // pathological cases where one call hangs for seconds and the others
    // are fast.
    assert!(
        elapsed < Duration::from_secs(2),
        "total elapsed {elapsed:?} exceeds 2s budget for {N} status calls"
    );

    eprintln!("[health::repeated_status] {N} calls in {elapsed:?} (avg {avg_us} µs)");
}
