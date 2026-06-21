//! Integration tests for the `db_stats` IPC verb.
//!
//! Uses the in-process `IpcServer` harness (same pattern as `import_handler.rs`)
//! so no pre-built binary is required and no global state is shared.
//!
//! Covered cases:
//!   1. `db_stats_empty_db` — fresh DB → item_count == 0, size_bytes is numeric.
//!   2. `db_stats_after_import` — insert N items via `import`, then call
//!      `db_stats` and assert item_count == N.
//!   3. `db_stats_response_fields_present` — response must contain exactly the
//!      two fields the UI expects: `item_count` and `size_bytes`.

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
// Harness
// ---------------------------------------------------------------------------

const TEST_LOCAL_KEY: [u8; 32] = [0x55u8; 32];

async fn start_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("db-stats-test.sock");

    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("open in-memory db"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode,
        Arc::new(zeroize::Zeroizing::new(TEST_LOCAL_KEY)),
        Arc::new([0u8; 32]),
    );

    let sock_for_task = sock.clone();
    tokio::spawn(async move {
        server
            .serve(&sock_for_task, tokio_util::sync::CancellationToken::new())
            .await
            .ok();
    });

    // Brief pause so the listener binds before the first connect.
    tokio::time::sleep(Duration::from_millis(50)).await;
    (dir, sock)
}

/// Send one newline-terminated request, receive one response line.
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

/// Build an `import` request that inserts N plaintext items.
fn import_request(id: &str, items: &[(&str, &[u8], i64)]) -> String {
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

/// On a completely fresh (empty) in-memory DB `db_stats` must return
/// `item_count == 0` and a numeric `size_bytes`.
#[tokio::test]
async fn db_stats_empty_db() {
    let (_dir, sock) = start_server().await;

    let resp = roundtrip(
        &sock,
        r#"{"id":"ds1","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "db_stats must return ok=true: {resp}");

    let data = &resp["data"];
    assert_eq!(
        data["item_count"], 0,
        "fresh DB must report item_count == 0: {resp}"
    );
    assert!(
        data["size_bytes"].is_number(),
        "size_bytes must be a number: {resp}"
    );
    // size_bytes may be 0 for an in-memory DB whose backing file doesn't exist.
    let size_bytes = data["size_bytes"].as_u64().expect("size_bytes is u64");
    // In-memory DB: the daemon path computes size from the *file* path which
    // doesn't exist → 0 is the correct sentinel. Just verify it's not negative
    // (which JSON unsigned can't be, but we document the expectation explicitly).
    let _ = size_bytes; // value is asserted to be numeric above
}

/// After importing N items `db_stats.item_count` must equal N.
///
/// This is the key correctness test: it proves the daemon handler actually
/// reads from the live DB and doesn't hardcode or cache a stale value.
#[tokio::test]
async fn db_stats_after_import_reflects_count() {
    let (_dir, sock) = start_server().await;

    const N: i64 = 5;
    let items: Vec<(&str, &[u8], i64)> = vec![
        ("text", b"first", 1_000_000),
        ("text", b"second", 2_000_000),
        ("text", b"third", 3_000_000),
        ("text", b"fourth", 4_000_000),
        ("text", b"fifth", 5_000_000),
    ];

    let import_resp = roundtrip(&sock, &import_request("imp-ds", &items)).await;
    assert_eq!(
        import_resp["ok"], true,
        "import must succeed before testing db_stats: {import_resp}"
    );
    assert_eq!(
        import_resp["data"]["inserted"], N,
        "all {N} items must be inserted: {import_resp}"
    );

    let resp = roundtrip(
        &sock,
        r#"{"id":"ds2","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "db_stats must return ok=true: {resp}");
    assert_eq!(
        resp["data"]["item_count"], N,
        "item_count must equal the number of imported items ({N}): {resp}"
    );
    assert!(
        resp["data"]["size_bytes"].is_number(),
        "size_bytes must be a number: {resp}"
    );
}

/// `db_stats` response must expose exactly the two fields the UI relies on:
/// `item_count` and `size_bytes`. Extra fields are allowed (forwards-compat),
/// but missing either field is a contract break.
#[tokio::test]
async fn db_stats_response_fields_present() {
    let (_dir, sock) = start_server().await;

    let resp = roundtrip(
        &sock,
        r#"{"id":"ds3","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "expected ok=true: {resp}");

    let data = resp["data"]
        .as_object()
        .expect("data must be a JSON object");

    assert!(
        data.contains_key("item_count"),
        "response.data must contain 'item_count': {resp}"
    );
    assert!(
        data.contains_key("size_bytes"),
        "response.data must contain 'size_bytes': {resp}"
    );

    // Both must be numeric (not null, not string, not bool).
    assert!(
        resp["data"]["item_count"].is_number(),
        "item_count must be numeric: {resp}"
    );
    assert!(
        resp["data"]["size_bytes"].is_number(),
        "size_bytes must be numeric: {resp}"
    );
}

/// `db_stats` must survive concurrent calls — ten clients asking simultaneously
/// must all receive `ok=true`. Proves the handler doesn't hold a lock that
/// would cause callers to starve or return errors under mild concurrency.
#[tokio::test]
async fn db_stats_concurrent_calls_all_succeed() {
    let (_dir, sock) = start_server().await;

    let sock = Arc::new(sock);
    let mut handles = Vec::with_capacity(10);

    for i in 0..10_u8 {
        let sock = Arc::clone(&sock);
        handles.push(tokio::spawn(async move {
            let req = format!(
                r#"{{"id":"c{i}","method":"db_stats","protocol_version":1,"params":{{}}}}"#
            );
            let resp = roundtrip(&sock, &req).await;
            (i, resp)
        }));
    }

    for h in handles {
        let (i, resp) = h.await.expect("task must not panic");
        assert_eq!(
            resp["ok"], true,
            "concurrent client {i} got non-ok response: {resp}"
        );
        assert!(
            resp["data"]["item_count"].is_number(),
            "client {i} item_count must be numeric: {resp}"
        );
    }
}
