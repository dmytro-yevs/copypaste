//! Integration tests for the daemon's `import` IPC method.
//!
//! Each test spins up an `IpcServer` backed by an in-memory SQLite database,
//! binds a per-test temp socket, sends one or more newline-delimited JSON
//! requests, and asserts on the parsed responses.
//!
//! Covered cases (per the beta `daemon-import` task spec):
//!   1. `import_inserts_new_items` — 3 fresh items in → 3 inserted, 0 skipped.
//!   2. `import_dedupes_existing` — same item imported twice → second pass
//!      reports 0 inserted, 1 skipped.
//!   3. `import_handles_empty_list` — empty `items` array → 0/0, no error.
//!   4. `import_malformed_b64_returns_error` — invalid base64 → typed
//!      `invalid_argument` error response.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use copypaste_core::Database;
use copypaste_daemon::ipc::IpcServer;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// Start an `IpcServer` on a fresh temp socket and return the socket path
/// (kept alive by the returned `TempDir`).
async fn start_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("import-test.sock");

    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("open in-memory db"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(db, private_mode);

    let sock_for_task = sock.clone();
    tokio::spawn(async move {
        server.serve(&sock_for_task).await.ok();
    });

    // Give the listener a moment to bind before the first connect attempt.
    tokio::time::sleep(Duration::from_millis(50)).await;
    (dir, sock)
}

/// Send a single newline-terminated request, read one response line back.
async fn roundtrip(sock: &std::path::Path, request: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(sock).await.expect("connect");
    let mut payload = request.to_string();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).await.expect("write");

    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines
        .next_line()
        .await
        .expect("read")
        .expect("daemon closed without reply");
    serde_json::from_str(&line).expect("response is valid JSON")
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Build a valid `import` request JSON string from a list of
/// `(content_type, raw_bytes, created_at_ms)` tuples.
fn build_import_request(id: &str, items: &[(&str, &[u8], i64)]) -> String {
    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|(ct, bytes, ts)| {
            serde_json::json!({
                "content_type": ct,
                "content_bytes_b64": b64(bytes),
                "created_at_ms": ts,
                "metadata": null,
            })
        })
        .collect();
    serde_json::json!({
        "id": id,
        "method": "import",
        "protocol_version": 1,
        "params": { "items": items_json },
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_inserts_new_items() {
    let (_dir, sock) = start_server().await;

    let req = build_import_request(
        "imp1",
        &[
            ("text", b"alpha", 1_000_000),
            ("text", b"beta", 2_000_000),
            ("text", b"gamma", 3_000_000),
        ],
    );
    let resp = roundtrip(&sock, &req).await;

    assert_eq!(resp["ok"], true, "expected ok=true, got: {resp}");
    assert_eq!(resp["data"]["inserted"], 3);
    assert_eq!(resp["data"]["skipped"], 0);

    // Sanity: a follow-up `list` should report 3 items in the page total.
    let list_req =
        r#"{"id":"l1","method":"list","protocol_version":1,"params":{"limit":10,"offset":0}}"#;
    let list_resp = roundtrip(&sock, list_req).await;
    assert_eq!(list_resp["ok"], true, "list must succeed: {list_resp}");
    assert_eq!(list_resp["data"]["total"], 3);
}

#[tokio::test]
async fn import_dedupes_existing() {
    let (_dir, sock) = start_server().await;

    // First import — should land.
    let payload = b"duplicate-content";
    let ts = 5_000_000_i64;
    let first = build_import_request("d1", &[("text", payload, ts)]);
    let resp1 = roundtrip(&sock, &first).await;
    assert_eq!(resp1["ok"], true, "first import must succeed: {resp1}");
    assert_eq!(resp1["data"]["inserted"], 1);
    assert_eq!(resp1["data"]["skipped"], 0);

    // Second import of the EXACT same content (same hash, same wall_time
    // bucket) — must be deduped.
    let second = build_import_request("d2", &[("text", payload, ts)]);
    let resp2 = roundtrip(&sock, &second).await;
    assert_eq!(resp2["ok"], true, "second import must succeed: {resp2}");
    assert_eq!(resp2["data"]["inserted"], 0);
    assert_eq!(resp2["data"]["skipped"], 1);
}

#[tokio::test]
async fn import_handles_empty_list() {
    let (_dir, sock) = start_server().await;

    let req = build_import_request("e1", &[]);
    let resp = roundtrip(&sock, &req).await;

    assert_eq!(resp["ok"], true, "empty import must succeed: {resp}");
    assert_eq!(resp["data"]["inserted"], 0);
    assert_eq!(resp["data"]["skipped"], 0);
}

#[tokio::test]
async fn import_malformed_b64_returns_error() {
    let (_dir, sock) = start_server().await;

    // Hand-craft the request with an obviously invalid base64 string
    // (`!!!!` contains characters outside the standard alphabet).
    let req = serde_json::json!({
        "id": "bad1",
        "method": "import",
        "protocol_version": 1,
        "params": {
            "items": [{
                "content_type": "text",
                "content_bytes_b64": "!!!!not-base64!!!!",
                "created_at_ms": 1_000_000,
                "metadata": null,
            }]
        }
    })
    .to_string();

    let resp = roundtrip(&sock, &req).await;
    assert_eq!(resp["ok"], false, "malformed base64 must fail: {resp}");
    assert_eq!(
        resp["error_code"], "invalid_argument",
        "expected stable error_code=invalid_argument, got: {resp}"
    );
    let err_msg = resp["error"].as_str().unwrap_or_default();
    assert!(
        err_msg.contains("base64"),
        "error message should mention base64, got: {err_msg}"
    );
}
