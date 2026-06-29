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

/// The non-zero local storage key the test server is constructed with. Using
/// a non-zero key (rather than `[0u8; 32]`) makes the round-trip read tests
/// meaningful: the daemon must encrypt imported/captured content under *this*
/// key and AAD, and the read-back must use the matching derived key.
const TEST_LOCAL_KEY: [u8; 32] = [0x42u8; 32];

/// Start an `IpcServer` on a fresh temp socket and return the socket path
/// (kept alive by the returned `TempDir`).
async fn start_server() -> (tempfile::TempDir, std::path::PathBuf) {
    let (dir, sock, _db) = start_server_returning_db().await;
    (dir, sock)
}

/// Like [`start_server`] but also hands back the shared `Database` so a test
/// can read stored rows and verify they round-trip through the production
/// read crypto.
async fn start_server_returning_db() -> (tempfile::TempDir, std::path::PathBuf, Arc<Mutex<Database>>)
{
    let dir = tempdir().expect("tempdir");
    let sock = dir.path().join("import-test.sock");

    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("open in-memory db"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let server = IpcServer::new(
        db.clone(),
        private_mode,
        std::sync::Arc::new(zeroize::Zeroizing::new(TEST_LOCAL_KEY)),
        std::sync::Arc::new([0u8; 32]),
    );

    let sock_for_task = sock.clone();
    tokio::spawn(async move {
        server
            .serve(&sock_for_task, tokio_util::sync::CancellationToken::new())
            .await
            .ok();
    });

    // Give the listener a moment to bind before the first connect attempt.
    tokio::time::sleep(Duration::from_millis(50)).await;
    (dir, sock, db)
}

/// Read a stored text row back through the EXACT crypto the daemon's read
/// path (`ipc::write_to_pasteboard`, text branch) uses: derive
/// `v2_key = derive_v2(local_key)`, then dispatch on the row's `key_version`
/// via `decrypt_item_by_version` with `v1_key = local_key` directly and the
/// AAD rebuilt from the row's `item_id`.
///
/// This is the production read crypto — the `copy`/`paste` IPC verb funnels
/// through the same `decrypt_item_by_version` call — so a successful decrypt
/// here proves the stored row is genuinely retrievable, without writing to
/// the real NSPasteboard.
fn read_text_row_via_read_path(
    db: &Database,
    local_key: &[u8; 32],
    row_id: &str,
) -> Result<Vec<u8>, copypaste_core::EncryptError> {
    let (item_id, content, nonce_vec, key_version): (String, Vec<u8>, Vec<u8>, i64) = db
        .conn()
        .query_row(
            "SELECT item_id, content, content_nonce, key_version \
             FROM clipboard_items WHERE id = ?1",
            rusqlite::params![row_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("row exists");
    let nonce: [u8; copypaste_core::NONCE_SIZE] =
        nonce_vec.as_slice().try_into().map_err(|_| {
            // A wrong-length nonce can never decrypt — surface it as AuthFailed so
            // the RED test (verbatim store with empty nonce) fails decryption
            // rather than panicking, matching the read path's own length guard.
            copypaste_core::EncryptError::AuthFailed
        })?;
    let v1_key = *local_key;
    let v2_key = copypaste_core::derive_v2(local_key);
    copypaste_core::decrypt_item_by_version(
        key_version as u8,
        copypaste_core::V1Key(&v1_key),
        copypaste_core::V2Key(&v2_key),
        &item_id,
        &nonce,
        &content,
    )
}

/// Fetch the single row id present in the DB (tests insert exactly one).
fn only_row_id(db: &Database) -> String {
    db.conn()
        .query_row("SELECT id FROM clipboard_items LIMIT 1", [], |r| r.get(0))
        .expect("exactly one row present")
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

    // Sanity: a follow-up `history_page` should report 3 items in the page total.
    // (`list` was deprecated in c4q2.17 — same response shape, pinned items first.)
    let list_req = r#"{"id":"l1","method":"history_page","protocol_version":1,"params":{"limit":10,"offset":0}}"#;
    let list_resp = roundtrip(&sock, list_req).await;
    assert_eq!(
        list_resp["ok"], true,
        "history_page must succeed: {list_resp}"
    );
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

// ---------------------------------------------------------------------------
// Round-trip / read-path coverage (fix/import-and-rt-tests)
// ---------------------------------------------------------------------------

/// SUSPECTED ACTIVE BUG (audit): the `import` handler stored imported content
/// VERBATIM via `ClipboardItem::new_text(bytes, Vec::new(), 0)` — i.e. the
/// plaintext bytes as `content`, an EMPTY `content_nonce`, and `key_version`
/// stamped to the current value (2). The production read path
/// (`decrypt_item_by_version`, dispatched by `copy`/`paste`) expects a real
/// XChaCha20-Poly1305 ciphertext + 24-byte nonce under the v2 key with the v4
/// AAD, so an imported row could never be decrypted/retrieved.
///
/// This drives the REAL import path through the IPC server, then reads the
/// stored row back through the REAL read crypto and asserts the original
/// plaintext returns.
#[tokio::test]
async fn imported_item_is_retrievable_via_copy() {
    let (_dir, sock, db) = start_server_returning_db().await;

    let plaintext = b"imported clipboard payload that must round-trip";
    let req = build_import_request("rt1", &[("text", plaintext, 7_000_000)]);
    let resp = roundtrip(&sock, &req).await;
    assert_eq!(resp["ok"], true, "import must succeed: {resp}");
    assert_eq!(resp["data"]["inserted"], 1, "one item imported: {resp}");

    // Read the stored row back through the production read crypto (the same
    // `decrypt_item_by_version` call the `copy`/`paste` IPC verb funnels
    // through). Before the fix this fails: the row holds verbatim plaintext +
    // an empty (0-length) nonce stamped key_version=2, so decryption errors.
    let guard = db.lock().await;
    let row_id = only_row_id(&guard);
    let recovered = read_text_row_via_read_path(&guard, &TEST_LOCAL_KEY, &row_id)
        .expect("imported item must be readable through the production read path");
    assert_eq!(
        recovered, plaintext,
        "imported item must round-trip back to the original bytes"
    );
}

// ---------------------------------------------------------------------------
// PG-26: sensitivity recompute — import cannot smuggle a credential as non-sensitive
// ---------------------------------------------------------------------------

/// PG-26 regression test: if a caller supplies `is_sensitive=false` for an item
/// whose plaintext is a high-confidence credential (AWS access key), the daemon
/// must RECOMPUTE sensitivity from the plaintext and store the row as
/// `is_sensitive=true` — so the TTL auto-wipe fires regardless of the caller flag.
///
/// This guards against a tampered export file or a malicious IPC client that sets
/// `is_sensitive=false` to make credentials persist indefinitely in storage
/// instead of being auto-wiped after the sensitive TTL.
#[tokio::test]
async fn import_credential_with_false_flag_is_stored_as_sensitive() {
    let (_dir, sock, db) = start_server_returning_db().await;

    // A realistic AWS access key — `is_sensitive_for_autowipe` gives this 0.99
    // confidence, well above the 0.70 floor.
    let credential = b"AKIAIOSFODNN7EXAMPLE";

    // Build an import request that claims `is_sensitive=false`.
    let items_json = serde_json::json!([{
        "content_type": "text",
        "content_bytes_b64": b64(credential),
        "created_at_ms": 9_000_000_i64,
        "is_sensitive": false,  // <- attacker/tampered flag
        "metadata": null,
    }]);
    let req = serde_json::json!({
        "id": "pg26",
        "method": "import",
        "protocol_version": 1,
        "params": { "items": items_json },
    })
    .to_string();

    let resp = roundtrip(&sock, &req).await;
    assert_eq!(resp["ok"], true, "import must succeed: {resp}");
    assert_eq!(resp["data"]["inserted"], 1);

    // Read the stored row's `is_sensitive` flag directly from the DB.
    let guard = db.lock().await;
    let stored_is_sensitive: bool = guard
        .conn()
        .query_row(
            "SELECT is_sensitive FROM clipboard_items LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("row must exist");

    assert!(
        stored_is_sensitive,
        "PG-26: credential imported with is_sensitive=false must be stored as is_sensitive=true \
         after recompute from plaintext"
    );
}

/// PG-26 complementary: a non-sensitive item with `is_sensitive=false` must NOT
/// be upgraded — we should only recompute, not blanket-flag everything.
#[tokio::test]
async fn import_non_sensitive_with_false_flag_stays_non_sensitive() {
    let (_dir, sock, db) = start_server_returning_db().await;

    // Clearly non-sensitive text: a plain greeting.
    let harmless = b"Hello, world! This is a test note.";

    let items_json = serde_json::json!([{
        "content_type": "text",
        "content_bytes_b64": b64(harmless),
        "created_at_ms": 9_100_000_i64,
        "is_sensitive": false,
        "metadata": null,
    }]);
    let req = serde_json::json!({
        "id": "pg26b",
        "method": "import",
        "protocol_version": 1,
        "params": { "items": items_json },
    })
    .to_string();

    let resp = roundtrip(&sock, &req).await;
    assert_eq!(resp["ok"], true, "import must succeed: {resp}");
    assert_eq!(resp["data"]["inserted"], 1);

    let guard = db.lock().await;
    let stored_is_sensitive: bool = guard
        .conn()
        .query_row(
            "SELECT is_sensitive FROM clipboard_items LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("row must exist");

    assert!(
        !stored_is_sensitive,
        "PG-26: non-sensitive plaintext with is_sensitive=false must remain non-sensitive"
    );
}

/// PG-26 complementary: a non-sensitive item with `is_sensitive=true` (e.g., a
/// legitimately flagged note) must NOT be downgraded — OR semantics preserved.
#[tokio::test]
async fn import_non_sensitive_text_with_true_flag_preserved_as_sensitive() {
    let (_dir, sock, db) = start_server_returning_db().await;

    // Harmless text, but caller asserts sensitive (e.g. manually marked by user).
    let harmless = b"My private journal entry.";

    let items_json = serde_json::json!([{
        "content_type": "text",
        "content_bytes_b64": b64(harmless),
        "created_at_ms": 9_200_000_i64,
        "is_sensitive": true,   // <- caller flagged
        "metadata": null,
    }]);
    let req = serde_json::json!({
        "id": "pg26c",
        "method": "import",
        "protocol_version": 1,
        "params": { "items": items_json },
    })
    .to_string();

    let resp = roundtrip(&sock, &req).await;
    assert_eq!(resp["ok"], true, "import must succeed: {resp}");
    assert_eq!(resp["data"]["inserted"], 1);

    let guard = db.lock().await;
    let stored_is_sensitive: bool = guard
        .conn()
        .query_row(
            "SELECT is_sensitive FROM clipboard_items LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("row must exist");

    assert!(
        stored_is_sensitive,
        "PG-26 OR semantics: item caller-flagged as sensitive must remain sensitive \
         even if the plaintext detector does not flag it"
    );
}

/// GAP closer: end-to-end proof that a stored text item is retrievable via the
/// real `copy`/`get` IPC verb, not merely countable in the `list` total.
///
/// We seed a row EXACTLY the way the daemon's capture path does (the same
/// crypto `import` now uses: v2 key + v4 AAD bound to (item_id, 4, 2),
/// stamped key_version=2 by `ClipboardItem::new_text`) by importing through
/// the real IPC `import` verb, then:
///   1. assert the real `copy` IPC verb RESOLVES the row (not `not_found`)
///      and does NOT report a decrypt/auth failure — proving the server's
///      read path can decrypt it, and
///   2. confirm the recovered bytes match via the production read crypto.
#[tokio::test]
async fn captured_item_then_get_returns_content() {
    let (_dir, sock, db) = start_server_returning_db().await;

    let plaintext = b"end-to-end retrievable content";
    let import_req = build_import_request("cap1", &[("text", plaintext, 8_000_000)]);
    let import_resp = roundtrip(&sock, &import_req).await;
    assert_eq!(import_resp["ok"], true, "store must succeed: {import_resp}");

    let row_id = {
        let guard = db.lock().await;
        only_row_id(&guard)
    };

    // Drive the REAL `copy` IPC verb. On success the response carries
    // `written: true`; on a decrypt failure it returns error_code=auth_failed;
    // on a missing row, not_found. The load-bearing assertion is that the
    // server RESOLVED the row and did NOT fail to decrypt it.
    let copy_req = serde_json::json!({
        "id": "cp1",
        "method": "copy",
        "protocol_version": 1,
        "params": { "id": row_id },
    })
    .to_string();
    let copy_resp = roundtrip(&sock, &copy_req).await;
    assert_ne!(
        copy_resp["error_code"], "not_found",
        "copy must resolve the stored row by id: {copy_resp}"
    );
    assert_ne!(
        copy_resp["error_code"], "auth_failed",
        "copy must be able to decrypt the stored row: {copy_resp}"
    );

    // Confirm the actual recovered bytes via the production read crypto.
    let guard = db.lock().await;
    let recovered = read_text_row_via_read_path(&guard, &TEST_LOCAL_KEY, &row_id)
        .expect("stored item must decrypt through the production read path");
    assert_eq!(
        recovered, plaintext,
        "retrieved content must equal the stored content"
    );
}
