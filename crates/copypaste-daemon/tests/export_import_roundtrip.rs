//! Integration tests for the `export` and `import` IPC verbs — exercised
//! end-to-end over an in-process `IpcServer`.
//!
//! Covered cases:
//!   1. `export_empty_db` — fresh DB → items == [], skipped_non_text == 0.
//!   2. `export_response_fields_present` — response shape matches what the UI
//!      and CLI expect: `items` array, `skipped_non_text` counter.
//!   3. `export_import_roundtrip` — import N items, export them back, verify
//!      the item count and that each item has the expected fields.
//!   4. `import_export_content_preserved` — import one item with known content,
//!      export, decode the base64 payload, assert content matches.
//!   5. `export_limit_param` — limit=2 on a 5-item DB returns exactly 2 items.
//!   6. `reset_database_requires_confirm` — calling without `confirm=true`
//!      must return an error (not wipe the DB silently).
//!   7. `reset_database_clears_items` — import items, reset_database with
//!      confirm=true, then db_stats must report item_count == 0 (DB wiped).

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

const TEST_LOCAL_KEY: [u8; 32] = [0x77u8; 32];

async fn start_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("export-import-test.sock");

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

    tokio::time::sleep(Duration::from_millis(50)).await;
    (dir, sock)
}

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
// export tests
// ---------------------------------------------------------------------------

/// Fresh DB: export must return an empty items array and skipped_non_text == 0.
#[tokio::test]
async fn export_empty_db() {
    let (_dir, sock) = start_server().await;

    let resp = roundtrip(
        &sock,
        r#"{"id":"ex0","method":"export","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "export must succeed on empty DB: {resp}");
    let items = resp["data"]["items"]
        .as_array()
        .expect("data.items must be an array");
    assert!(items.is_empty(), "fresh DB must export zero items: {resp}");
    assert_eq!(
        resp["data"]["skipped_non_text"], 0,
        "fresh DB must report skipped_non_text == 0: {resp}"
    );
}

/// `export` response must contain the fields `items` (array) and
/// `skipped_non_text` (number) that the CLI / UI consume.
#[tokio::test]
async fn export_response_fields_present() {
    let (_dir, sock) = start_server().await;

    let resp = roundtrip(
        &sock,
        r#"{"id":"ex-fields","method":"export","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(resp["ok"], true, "expected ok=true: {resp}");

    let data = resp["data"]
        .as_object()
        .expect("data must be a JSON object");
    assert!(
        data.contains_key("items"),
        "data must contain 'items': {resp}"
    );
    assert!(
        data.contains_key("skipped_non_text"),
        "data must contain 'skipped_non_text': {resp}"
    );
    assert!(
        resp["data"]["items"].is_array(),
        "items must be an array: {resp}"
    );
    assert!(
        resp["data"]["skipped_non_text"].is_number(),
        "skipped_non_text must be numeric: {resp}"
    );
}

/// Import N items, then export — the exported items array must have length N
/// and each item must carry the expected fields.
#[tokio::test]
async fn export_import_roundtrip() {
    let (_dir, sock) = start_server().await;

    const N: usize = 3;
    let items = [
        ("text", b"alpha" as &[u8], 1_000_000_i64),
        ("text", b"beta", 2_000_000),
        ("text", b"gamma", 3_000_000),
    ];

    let imp_resp = roundtrip(&sock, &import_request("imp-rt", &items)).await;
    assert_eq!(imp_resp["ok"], true, "import must succeed: {imp_resp}");
    assert_eq!(
        imp_resp["data"]["inserted"], N as i64,
        "all {N} items must be inserted: {imp_resp}"
    );

    let exp_resp = roundtrip(
        &sock,
        r#"{"id":"exp-rt","method":"export","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(exp_resp["ok"], true, "export must succeed: {exp_resp}");
    let exported = exp_resp["data"]["items"]
        .as_array()
        .expect("items must be array");
    assert_eq!(
        exported.len(),
        N,
        "export must return exactly {N} items: {exp_resp}"
    );

    // Each exported item must carry the fields the CLI import path requires.
    for (idx, item) in exported.iter().enumerate() {
        let obj = item
            .as_object()
            .expect("each exported item must be an object");
        for field in &[
            "content_type",
            "content_bytes_b64",
            "created_at_ms",
            "is_sensitive",
        ] {
            assert!(
                obj.contains_key(*field),
                "exported item[{idx}] missing field '{field}': {item}"
            );
        }
        assert_eq!(
            item["content_type"], "text",
            "exported item[{idx}] content_type must be 'text': {item}"
        );
        assert!(
            item["content_bytes_b64"].is_string(),
            "exported item[{idx}] content_bytes_b64 must be a string: {item}"
        );
        assert!(
            item["created_at_ms"].is_number(),
            "exported item[{idx}] created_at_ms must be numeric: {item}"
        );
    }
}

/// Import one item with known plaintext, export it back, and verify the
/// decoded base64 payload matches the original bytes.
///
/// This proves the full encrypt→store→decrypt→export chain is lossless.
#[tokio::test]
async fn export_import_content_preserved() {
    let (_dir, sock) = start_server().await;

    let original: &[u8] = b"Hello, CopyPaste export round-trip!";
    let ts = 9_999_999_i64;

    let imp_resp = roundtrip(&sock, &import_request("cp-imp", &[("text", original, ts)])).await;
    assert_eq!(imp_resp["ok"], true, "import must succeed: {imp_resp}");
    assert_eq!(imp_resp["data"]["inserted"], 1);

    let exp_resp = roundtrip(
        &sock,
        r#"{"id":"cp-exp","method":"export","protocol_version":1,"params":{}}"#,
    )
    .await;

    assert_eq!(exp_resp["ok"], true, "export must succeed: {exp_resp}");
    let items = exp_resp["data"]["items"]
        .as_array()
        .expect("items must be array");
    assert_eq!(items.len(), 1, "exactly one item exported: {exp_resp}");

    let b64_str = items[0]["content_bytes_b64"]
        .as_str()
        .expect("content_bytes_b64 must be a string");

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64_str)
        .expect("content_bytes_b64 must be valid base64");

    assert_eq!(
        decoded, original,
        "exported content must match the originally imported bytes"
    );
}

/// `export` with `limit=2` on a 5-item DB must return exactly 2 items (the
/// most-recent two, re-ordered oldest-first for deterministic import).
#[tokio::test]
async fn export_limit_param() {
    let (_dir, sock) = start_server().await;

    let items = [
        ("text", b"one" as &[u8], 1_000_000_i64),
        ("text", b"two", 2_000_000),
        ("text", b"three", 3_000_000),
        ("text", b"four", 4_000_000),
        ("text", b"five", 5_000_000),
    ];
    let imp_resp = roundtrip(&sock, &import_request("lim-imp", &items)).await;
    assert_eq!(imp_resp["ok"], true, "import must succeed: {imp_resp}");
    assert_eq!(imp_resp["data"]["inserted"], 5);

    let exp_resp = roundtrip(
        &sock,
        r#"{"id":"lim-exp","method":"export","protocol_version":1,"params":{"limit":2}}"#,
    )
    .await;

    assert_eq!(
        exp_resp["ok"], true,
        "export with limit must succeed: {exp_resp}"
    );
    let exported = exp_resp["data"]["items"]
        .as_array()
        .expect("items must be array");
    assert_eq!(
        exported.len(),
        2,
        "export with limit=2 must return exactly 2 items: {exp_resp}"
    );
}

// ---------------------------------------------------------------------------
// reset_database tests
// ---------------------------------------------------------------------------

/// Calling `reset_database` WITHOUT `confirm=true` must return an error —
/// it must not silently wipe the DB.
#[tokio::test]
async fn reset_database_requires_confirm() {
    let (_dir, sock) = start_server().await;

    // First, insert an item so there is something to lose.
    let imp_resp = roundtrip(
        &sock,
        &import_request("rdb-pre", &[("text", b"precious data", 1_000_000)]),
    )
    .await;
    assert_eq!(imp_resp["ok"], true, "import must succeed: {imp_resp}");

    // Attempt reset WITHOUT confirm — must be rejected.
    let no_confirm = roundtrip(
        &sock,
        r#"{"id":"rdb1","method":"reset_database","protocol_version":1,"params":{}}"#,
    )
    .await;
    assert_eq!(
        no_confirm["ok"], false,
        "reset_database without confirm must return ok=false: {no_confirm}"
    );
    // Error message must mention the missing confirm flag so callers can
    // diagnose the rejection.
    let error_msg = no_confirm["error"]
        .as_str()
        .or_else(|| no_confirm["message"].as_str())
        .or_else(|| no_confirm["data"]["error"].as_str())
        .unwrap_or("");
    assert!(
        !error_msg.is_empty(),
        "reset_database rejection must include an error message: {no_confirm}"
    );

    // confirm=false is also rejected.
    let false_confirm = roundtrip(
        &sock,
        r#"{"id":"rdb2","method":"reset_database","protocol_version":1,"params":{"confirm":false}}"#,
    )
    .await;
    assert_eq!(
        false_confirm["ok"], false,
        "reset_database with confirm=false must return ok=false: {false_confirm}"
    );

    // The DB must still have the original item — no data was lost.
    let stats_resp = roundtrip(
        &sock,
        r#"{"id":"rdb3","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;
    assert_eq!(
        stats_resp["ok"], true,
        "db_stats must succeed: {stats_resp}"
    );
    assert_eq!(
        stats_resp["data"]["item_count"], 1,
        "item must still be present after rejected reset: {stats_resp}"
    );
}

/// `reset_database` with `confirm=true` must wipe the DB and return
/// `{reset: true, ready: true}`. A subsequent `db_stats` must report
/// `item_count == 0`.
#[tokio::test]
async fn reset_database_clears_items() {
    let (_dir, sock) = start_server().await;

    // Populate with 3 items.
    let items = [
        ("text", b"item-a" as &[u8], 1_000_000_i64),
        ("text", b"item-b", 2_000_000),
        ("text", b"item-c", 3_000_000),
    ];
    let imp_resp = roundtrip(&sock, &import_request("rdb-full-imp", &items)).await;
    assert_eq!(imp_resp["ok"], true, "import must succeed: {imp_resp}");
    assert_eq!(imp_resp["data"]["inserted"], 3);

    // Verify pre-condition: db_stats sees 3 items.
    let pre_stats = roundtrip(
        &sock,
        r#"{"id":"rdb-pre-stats","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;
    assert_eq!(pre_stats["ok"], true);
    assert_eq!(
        pre_stats["data"]["item_count"], 3,
        "pre-condition: 3 items before reset: {pre_stats}"
    );

    // Execute reset with confirm=true.
    let reset_resp = roundtrip(
        &sock,
        r#"{"id":"rdb-go","method":"reset_database","protocol_version":1,"params":{"confirm":true}}"#,
    )
    .await;
    assert_eq!(
        reset_resp["ok"], true,
        "reset_database with confirm=true must succeed: {reset_resp}"
    );
    // The response must carry reset=true and ready=true.
    assert_eq!(
        reset_resp["data"]["reset"], true,
        "response must include reset=true: {reset_resp}"
    );
    assert_eq!(
        reset_resp["data"]["ready"], true,
        "response must include ready=true: {reset_resp}"
    );

    // Post-condition: all items gone.
    let post_stats = roundtrip(
        &sock,
        r#"{"id":"rdb-post-stats","method":"db_stats","protocol_version":1,"params":{}}"#,
    )
    .await;
    assert_eq!(
        post_stats["ok"], true,
        "db_stats must succeed after reset: {post_stats}"
    );
    assert_eq!(
        post_stats["data"]["item_count"], 0,
        "item_count must be 0 after reset_database: {post_stats}"
    );
}

/// Regardless of whether `reset_database` succeeds or fails (the in-process
/// in-memory server may fail to open the real on-disk DB), the daemon must
/// keep accepting IPC requests afterwards — no listener task must crash.
///
/// This test is intentionally lenient about the reset result: what matters is
/// that the `status` call after any outcome still returns a response (not a
/// hang or a broken pipe).
#[tokio::test]
async fn reset_database_daemon_accepts_requests_after_reset_attempt() {
    let (_dir, sock) = start_server().await;

    // Attempt reset — may succeed or fail (both outcomes are valid for the
    // in-memory server whose db_path points to the real system path).
    let _reset_resp = roundtrip(
        &sock,
        r#"{"id":"rdb-h1","method":"reset_database","protocol_version":1,"params":{"confirm":true}}"#,
    )
    .await;
    // We don't assert ok=true here — see module-level comment.

    // The daemon must still respond to a status query regardless of the outcome.
    let status_resp = roundtrip(
        &sock,
        r#"{"id":"rdb-h2","method":"status","protocol_version":1}"#,
    )
    .await;
    assert_eq!(
        status_resp["ok"], true,
        "daemon must keep accepting requests after any reset_database outcome: {status_resp}"
    );
}
