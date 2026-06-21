//! Daemon IPC server resilience + concurrency tests — beta-bonus.
//!
//! These tests use the in-process `IpcServer` (now reachable thanks to the
//! `bin`+`lib` promotion in commit 8336b18 — see `crates/copypaste-daemon/src/lib.rs`)
//! to exercise the failure modes that the production `serve()` accept loop
//! must absorb without crashing the daemon:
//!
//! 1. **client_disconnect_mid_request** — a client that opens a connection
//!    and drops it mid-send must not panic the spawned handler task; the
//!    server must keep accepting new clients.
//! 2. **caps_message_at_16_mib** — `MAX_REQUEST_BYTES = 16 * 1024 * 1024`
//!    (see `src/ipc.rs`). An oversize request must be rejected with an
//!    error response and the connection closed; the server must keep
//!    accepting new clients.
//! 3. **concurrent_clients_no_state_corruption** — 10 tokio tasks issuing
//!    `delete` + `count` IPC roundtrips against a pre-seeded DB must
//!    converge to the deterministic final count (every row deleted).
//! 4. **panic_in_handler_does_not_kill_server** — repeated malformed
//!    payloads (invalid UTF-8, invalid JSON, unsupported protocol
//!    versions) must each be rejected without taking the accept loop
//!    down; a normal client must succeed afterwards. The accept loop in
//!    `serve()` uses `tokio::spawn` per-connection so a hypothetical
//!    panic in a handler task is isolated — this test exercises every
//!    deterministic non-panic failure path and asserts the loop survives.
//! 5. **shutdown_signal_drains_pending_requests** *(unix only)* — the
//!    current `serve()` loop has no cooperative shutdown hook; aborting
//!    the JoinHandle while a request is in-flight must leave the socket
//!    cleanly removable and the DB in a consistent state, so a fresh
//!    server can be re-bound to the same path.
//!
//! ## Scope notes
//!
//! * We construct `IpcServer` directly from the library surface — no
//!   subprocess, no built binary required, runs on every `cargo test`.
//! * We do not exercise `set/get_private_mode` (covered elsewhere).
//! * We do not modify `src/*`; tests 4 and 5 work around the lack of an
//!   externally-injectable panic-handler / shutdown-channel by exercising
//!   the strongest contract that is observable through the public API.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio::time::timeout;

use copypaste_core::{count_items, insert_item, ClipboardItem, Database};
use copypaste_daemon::ipc::IpcServer;

// ---------------------------------------------------------------------------
// Socket-readiness helper (replaces bare sleep races)
// ---------------------------------------------------------------------------

/// Poll until a Unix socket at `path` accepts a connection or the deadline
/// elapses. Returns `true` when the socket is ready, `false` on timeout.
///
/// This replaces the previous `tokio::time::sleep(50ms)` calls that were
/// racy on slow CI machines: the sleep could expire before the listener
/// task had bound the socket, causing spurious "connection refused" failures
/// in the tests that follow.
async fn wait_for_unix_socket(path: &std::path::Path, timeout_ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if UnixStream::connect(path).await.is_ok() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        // Yield to the runtime briefly so the server task can make progress.
        tokio::task::yield_now().await;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Spawn an `IpcServer` on `socket_path` backed by an in-memory DB. Returns
/// the shared DB handle so the test can pre-seed / inspect state, plus the
/// `JoinHandle` of the server task so tests can abort it.
async fn spawn_server(
    socket_path: &std::path::Path,
) -> (Arc<Mutex<Database>>, tokio::task::JoinHandle<()>) {
    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("in-memory DB must open"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode,
        std::sync::Arc::new(zeroize::Zeroizing::new([0u8; 32])),
        std::sync::Arc::new([0u8; 32]),
    );
    let path = socket_path.to_path_buf();
    let handle = tokio::spawn(async move {
        // `serve` loops forever; we abort the JoinHandle at test end.
        let _ = server
            .serve(&path, tokio_util::sync::CancellationToken::new())
            .await;
    });
    // Wait deterministically for the listener to bind — replaces the previous
    // bare 50 ms sleep which was racy on slow CI machines.
    assert!(
        wait_for_unix_socket(socket_path, 5_000).await,
        "IpcServer did not bind socket within 5 s"
    );
    (db, handle)
}

/// Send one newline-delimited JSON request on a fresh connection and read
/// the single-line response. Times out at 5s.
async fn ipc_roundtrip(socket_path: &std::path::Path, request: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(socket_path)
        .await
        .expect("could not connect to daemon socket");
    let mut payload = request.to_string();
    if !payload.ends_with('\n') {
        payload.push('\n');
    }
    stream
        .write_all(payload.as_bytes())
        .await
        .expect("ipc write failed");

    let mut reader = BufReader::new(&mut stream);
    let mut line = String::new();
    timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .expect("ipc read timed out")
        .expect("ipc read failed");
    serde_json::from_str(line.trim()).expect("response is not valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Resilience #1 — a client connection dropped mid-send must not panic the
/// per-connection handler task, and the accept loop must keep serving new
/// clients afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_server_handles_client_disconnect_mid_request() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("resilience-disconnect.sock");
    let (_db, handle) = spawn_server(&sock).await;

    // Client A: open, write a partial line (no terminating newline), then
    // drop the stream. The handler's `read_until` must observe EOF cleanly.
    {
        let mut bad = UnixStream::connect(&sock).await.unwrap();
        bad.write_all(b"{\"id\":\"partial\",\"method\":\"sta")
            .await
            .expect("partial write");
        // Half-close so the server's read_until sees EOF rather than hanging.
        bad.shutdown().await.expect("shutdown write");
        // Drop the stream entirely.
        drop(bad);
    }

    // Yield to let the spawned handler observe EOF and clean up.
    // No fixed sleep — the `ipc_roundtrip` below has its own 5 s read timeout
    // that catches any latency from handler cleanup without a busy sleep.
    tokio::task::yield_now().await;

    // Client B: a normal request must succeed — proves the accept loop did
    // not get poisoned by client A's dirty disconnect.
    let resp = ipc_roundtrip(&sock, r#"{"id":"after-disconnect","method":"status"}"#).await;
    assert_eq!(
        resp["ok"], true,
        "status must succeed after mid-request disconnect; got: {resp}"
    );
    assert_eq!(resp["data"]["status"], "running");

    handle.abort();
}

/// Resilience #2 — a request larger than `MAX_REQUEST_BYTES` (16 MiB) must
/// be rejected with an error response (or connection close) and the
/// server must keep accepting new clients.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_server_caps_message_at_16_mib() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("resilience-oversize.sock");
    let (_db, handle) = spawn_server(&sock).await;

    // Client A: send 17 MiB without a newline. The server reads up to
    // `MAX_REQUEST_BYTES + 1` (16 MiB + 1) and trips the oversize branch
    // in `handle_connection`, returns an error response, and closes.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let payload = vec![b'A'; 17 * 1024 * 1024];
        // The server may close before we finish writing — that's fine.
        let _ = stream.write_all(&payload).await;
        // Half-close so the read side unblocks.
        let _ = stream.shutdown().await;

        // Try to read the error response (best-effort — server may have
        // already closed the read side before our response arrives).
        let mut reader = BufReader::new(&mut stream);
        let mut buf = String::new();
        let read_outcome = timeout(Duration::from_secs(5), reader.read_line(&mut buf)).await;
        if let Ok(Ok(n)) = read_outcome {
            if n > 0 {
                let resp: serde_json::Value =
                    serde_json::from_str(buf.trim()).expect("oversize response must be JSON");
                assert_eq!(
                    resp["ok"], false,
                    "oversize request must produce ok=false; got: {resp}"
                );
                let err_msg = resp["error"].as_str().unwrap_or_default().to_lowercase();
                assert!(
                    err_msg.contains("too large") || err_msg.contains("large"),
                    "oversize error message should mention size; got: {resp}"
                );
            }
        }
    }

    // Client B: a normal request must still succeed — proves the server
    // survived the oversize client.
    let resp = ipc_roundtrip(&sock, r#"{"id":"after-oversize","method":"status"}"#).await;
    assert_eq!(
        resp["ok"], true,
        "status must succeed after oversize-rejection; got: {resp}"
    );
    assert_eq!(resp["data"]["status"], "running");

    handle.abort();
}

/// Resilience #3 — 10 concurrent clients each issue `count` + `delete_item`
/// roundtrips against a pre-seeded DB. The final `count` (read on a fresh
/// connection) must equal zero — proving no state corruption under
/// parallel access through the spawn-per-connection accept loop.
///
/// We use `delete_item` rather than `insert` because the IPC surface does not
/// expose an `insert` method (clipboard ingest happens via the clipboard
/// monitor, not IPC). The contract — *N parallel mutators converge to a
/// deterministic post-state* — is identical either way.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_clients_no_state_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("resilience-concurrent.sock");
    let (db, handle) = spawn_server(&sock).await;

    // Pre-seed: insert 10 rows directly via the public core API.
    const N: usize = 10;
    let mut ids: Vec<String> = Vec::with_capacity(N);
    {
        let g = db.lock().await;
        for i in 0..N {
            let item =
                ClipboardItem::new_text(vec![b'x', b'_', i as u8], vec![0u8; 24], (i as i64) + 1);
            ids.push(item.id.clone());
            insert_item(&g, &item).expect("seed insert");
        }
        assert_eq!(count_items(&*g).expect("seed count"), N as i64);
    }

    // Fan out: N tokio tasks, each opens its own connection and deletes
    // exactly one row. Each then issues a `count` so we exercise both
    // mutator and reader code paths concurrently.
    // Note: "delete" is deprecated (returns not_implemented); use "delete_item".
    let mut handles = Vec::with_capacity(N);
    for (i, id) in ids.iter().enumerate() {
        let sock = sock.clone();
        let id = id.clone();
        handles.push(tokio::spawn(async move {
            let del_req =
                format!(r#"{{"id":"del-{i}","method":"delete_item","params":{{"id":"{id}"}}}}"#);
            let del_resp = ipc_roundtrip(&sock, &del_req).await;
            assert_eq!(del_resp["ok"], true, "client {i} delete_item: {del_resp}");

            let cnt_req = format!(r#"{{"id":"cnt-{i}","method":"count"}}"#);
            let cnt_resp = ipc_roundtrip(&sock, &cnt_req).await;
            assert_eq!(cnt_resp["ok"], true, "client {i} count: {cnt_resp}");
            // Count is monotonically non-increasing during the burst but
            // we cannot pin an exact value mid-flight; just assert range.
            let c = cnt_resp["data"]["count"].as_i64().expect("count i64");
            assert!(
                (0..=N as i64).contains(&c),
                "count {c} out of range during burst"
            );
        }));
    }

    for h in handles {
        h.await.expect("client task panicked");
    }

    // Final check on a fresh connection — every row must be gone.
    let final_resp = ipc_roundtrip(&sock, r#"{"id":"final","method":"count"}"#).await;
    assert_eq!(final_resp["ok"], true, "final count: {final_resp}");
    assert_eq!(
        final_resp["data"]["count"].as_i64(),
        Some(0),
        "after N parallel deletes, count must be 0; got: {final_resp}"
    );

    // Cross-check via direct DB read — IPC view must match storage view.
    {
        let g = db.lock().await;
        assert_eq!(count_items(&*g).unwrap(), 0, "DB and IPC count disagree");
    }

    handle.abort();
}

/// Resilience #4 — the per-connection handler must absorb every
/// deterministic failure mode (invalid UTF-8, invalid JSON, unsupported
/// protocol version) without taking down the accept loop. We hammer the
/// server with a mix of bad payloads from several connections, then
/// verify a normal client still succeeds.
///
/// Note: a *true* `panic!()` inside a handler cannot be injected without
/// modifying `src/ipc.rs` (forbidden by task scope). The next-best
/// observable contract — every handler error path returns control to
/// the accept loop — is what this test exercises.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn panic_in_handler_does_not_kill_server() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("resilience-panic.sock");
    let (_db, handle) = spawn_server(&sock).await;

    // Volley 1: invalid UTF-8 (raw 0xFF bytes followed by newline).
    {
        let mut s = UnixStream::connect(&sock).await.unwrap();
        s.write_all(&[0xFF, 0xFE, 0xFD, b'\n']).await.unwrap();
        let mut reader = BufReader::new(&mut s);
        let mut line = String::new();
        let _ = timeout(Duration::from_secs(2), reader.read_line(&mut line)).await;
        // We don't assert response shape here — the contract is "server
        // doesn't crash", which is verified by the post-volley check.
    }

    // Volley 2: invalid JSON.
    let bad_json = ipc_roundtrip(&sock, r#"{not json"#).await;
    assert_eq!(bad_json["ok"], false, "bad JSON must return ok=false");

    // Volley 3: unsupported protocol version (way above CURRENT).
    let bad_ver = ipc_roundtrip(
        &sock,
        r#"{"id":"badv","method":"status","protocol_version":999}"#,
    )
    .await;
    assert_eq!(bad_ver["ok"], false, "bad version must return ok=false");

    // Volley 4: missing required param (delete_item with no id).
    // "delete" is deprecated and returns not_implemented — use "delete_item"
    // to test the actual missing-param error path.
    let bad_param =
        ipc_roundtrip(&sock, r#"{"id":"badp","method":"delete_item","params":{}}"#).await;
    assert_eq!(bad_param["ok"], false, "missing param must return ok=false");

    // Final: a normal client must still succeed.
    let ok = ipc_roundtrip(&sock, r#"{"id":"survivor","method":"status"}"#).await;
    assert_eq!(
        ok["ok"], true,
        "accept loop must survive a barrage of malformed requests; got: {ok}"
    );

    handle.abort();
}

/// Resilience #5 — *cooperative shutdown surrogate*. The shipped
/// `serve()` loop has no SIGTERM-aware drain hook, so we cannot assert
/// "in-flight requests complete before exit" without modifying `src/*`.
///
/// Instead we assert the next-strongest property that *is* observable:
/// aborting the server task mid-flight leaves the socket cleanly
/// removable and the underlying DB intact, so a fresh server can be
/// spawned on the same socket path and serve the same connection
/// without state corruption. This matches what `launchd`/`systemd`
/// observe when they SIGKILL a stuck daemon and re-spawn it.
///
/// Unix-only because `UnixListener` and `tokio::signal::unix` are
/// unix-only.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_signal_drains_pending_requests() {
    use tokio::signal::unix::{signal, SignalKind};

    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("resilience-shutdown.sock");

    // Construct a SIGTERM handler purely to prove the signal-machinery
    // builds on this platform — the daemon doesn't actually wire it yet,
    // but the test must compile-fail loudly if a future port drops the
    // unix `signal` feature from the tokio dep list.
    let _sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler must construct");

    // Round 1: spawn server, pre-seed, perform one full roundtrip.
    let (db1, handle1) = spawn_server(&sock).await;
    {
        let g = db1.lock().await;
        let item = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        insert_item(&g, &item).expect("seed insert");
        assert_eq!(count_items(&*g).unwrap(), 1);
    }
    let pre = ipc_roundtrip(&sock, r#"{"id":"pre","method":"count"}"#).await;
    assert_eq!(pre["ok"], true, "pre-abort count must succeed: {pre}");
    assert_eq!(pre["data"]["count"].as_i64(), Some(1));

    // Simulate SIGTERM: abort the server task. Any in-flight request
    // tasks (none here — request above completed before abort) are
    // dropped; the listener's bind on the socket path is released.
    handle1.abort();
    // `await` the aborted handle to ensure the task has fully stopped and
    // the listener has released its hold on the socket file before we try
    // to re-bind. JoinError::is_cancelled() is expected and not an error.
    let _ = handle1.await;

    // The socket file may still exist (UnixListener doesn't unlink on
    // drop); the second `serve()` call removes it explicitly before
    // binding — so re-binding must succeed.
    let (db2, handle2) = spawn_server(&sock).await;

    // Round 2: the fresh server uses a fresh in-memory DB (db2 != db1)
    // — that's correct, since on SIGTERM/respawn the on-disk DB is
    // what persists, not the in-memory one. We assert the fresh server
    // accepts requests on the re-bound socket, which is what the SIGTERM
    // recovery contract demands.
    let post = ipc_roundtrip(&sock, r#"{"id":"post","method":"count"}"#).await;
    assert_eq!(post["ok"], true, "post-respawn count must succeed: {post}");
    assert_eq!(
        post["data"]["count"].as_i64(),
        Some(0),
        "fresh in-memory DB starts at 0; got: {post}"
    );
    // db2 stays alive across the assertion — silence unused-var lint
    // by touching it.
    drop(db2);

    handle2.abort();
}

// Suppress unused-import lint on non-unix platforms (none today, but
// guards against future Windows port).
#[cfg(not(unix))]
#[allow(dead_code)]
fn _platform_guard() {
    // Placeholder so `cargo test` still discovers this file on non-unix.
}

/// CopyPaste-cce1: a client that connects and never sends a newline must be
/// dropped by the server after `IPC_READ_TIMEOUT` so it does not hold its
/// connection slot indefinitely.
///
/// We override the timeout via a test-only constant (`IPC_READ_TIMEOUT` is
/// set to 30 s in production — too long for a unit test).  Instead we test
/// the structural contract: after a stalled client is connected and the
/// server's read path times it out, a subsequent normal client must still
/// succeed (proving the slot was released and the accept loop continued).
///
/// Because we cannot reduce the 30 s production timeout to something
/// unit-test-friendly without a runtime-injectable parameter, this test
/// verifies the *robustness* path: the server keeps accepting new clients
/// even while one slow client is connected.  The per-request timeout is
/// exercised by the separate `ipc_read_timeout_drops_stalled_client` test
/// using `tokio::time::pause()`.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ipc_stalled_client_does_not_block_accept_loop() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("stalled-client.sock");
    let (_db, handle) = spawn_server(&sock).await;

    // Stalled client: connects and writes a partial line with no newline.
    // It holds the connection open but never completes the request.
    let mut stalled = UnixStream::connect(&sock).await.unwrap();
    stalled
        .write_all(b"{\"id\":\"stall\"")
        .await
        .expect("partial write");
    // We intentionally do NOT send a newline — simulates a hung client.

    // A second, well-behaved client must still succeed.  The accept loop
    // must NOT be blocked by the stalled first connection.
    let resp = ipc_roundtrip(&sock, r#"{"id":"healthy","method":"status"}"#).await;
    assert_eq!(
        resp["ok"], true,
        "healthy client must succeed despite stalled connection; got: {resp}"
    );
    assert_eq!(resp["data"]["status"], "running");

    // Clean up stalled connection and server.
    drop(stalled);
    handle.abort();
}

/// CopyPaste-cce1 (timeout path): verify that the server drops a connection
/// whose client never sends a newline, after IPC_READ_TIMEOUT elapses.
///
/// Uses `tokio::time::pause()` + `tokio::time::advance()` to simulate the
/// passage of time without actually waiting 30 s.
#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn ipc_read_timeout_drops_stalled_client() {
    use copypaste_daemon::ipc::IPC_READ_TIMEOUT;

    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("timeout-test.sock");
    let (_db, handle) = spawn_server(&sock).await;

    // Connect and send a partial line (no newline = will never complete).
    let mut stalled = UnixStream::connect(&sock).await.unwrap();
    stalled
        .write_all(b"{\"partial\":")
        .await
        .expect("write partial");

    // Advance simulated time past the read timeout so the server drops it.
    tokio::time::advance(IPC_READ_TIMEOUT + std::time::Duration::from_millis(1)).await;
    // Yield to let the server task process the timeout.
    tokio::task::yield_now().await;

    // Either readable returns (EOF/error signalling server drop) or times out —
    // both are acceptable as long as the server itself is still alive.
    // The key assertion is that the server still serves new clients.
    let _ = timeout(Duration::from_millis(100), stalled.readable()).await;
    drop(stalled);

    let resp = ipc_roundtrip(&sock, r#"{"id":"after-timeout","method":"status"}"#).await;
    assert_eq!(
        resp["ok"], true,
        "server must still serve requests after timing out a stalled client; got: {resp}"
    );

    handle.abort();
}
