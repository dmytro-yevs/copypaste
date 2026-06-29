use super::*;
use crate::sync_common::{
    decode_cloud_file_payload, decode_payload_ct, encode_cloud_file_payload,
    wrap_and_check_cloud_upload_plaintext, CLOUD_FILE_HEADER_VERSION, CLOUD_FILE_LEGACY_MIME,
    CLOUD_FILE_LEGACY_NAME,
};
use base64::Engine as _;
use copypaste_core::{decrypt_from_cloud, derive_sync_key, encrypt_for_cloud, ClipboardItem};
use copypaste_supabase::auth::AuthClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex as AsyncMutex, RwLock};

/// A minimal, BYTEA-FAITHFUL fake PostgREST for `clipboard_items`.
///
/// Emulates the one Postgres property the old mocks lacked:
///   * On INSERT (`POST`), the JSON `payload_ct` string is interpreted with
///     Postgres `bytea` INPUT semantics:
///       - `"\x<hex>"`  → store the DECODED hex bytes (the daemon's correct
///         path via `encode_payload_ct_hex`);
///       - anything else → store the RAW ASCII BYTES of the string verbatim
///         (models the Android regression that sent bare base64 text, which
///         Postgres stored as the literal ASCII of that base64).
///   * On SELECT (`GET`), `payload_ct` is ALWAYS rendered as `"\x<hex>"` of
///     the stored bytes — PostgREST's hex OUTPUT form — no matter how it was
///     written. This asymmetry is exactly what hid the bug.
struct FakePostgrest {
    /// id -> stored row (raw bytea bytes + scalar columns echoed back).
    rows: Arc<AsyncMutex<HashMap<String, StoredRow>>>,
}

#[derive(Clone)]
struct StoredRow {
    item_id: String,
    content_type: String,
    payload_ct_bytes: Vec<u8>,
    lamport_ts: i64,
    wall_time: i64,
    device_id: String,
}

/// Decode a JSON `payload_ct` string under Postgres `bytea` INPUT rules.
/// `\x<hex>` → decoded bytes; anything else → the literal ASCII bytes of the
/// string (the regression path).
fn bytea_input(s: &str) -> Vec<u8> {
    if let Some(hexpart) = s.strip_prefix("\\x") {
        if let Ok(bytes) = hex::decode(hexpart) {
            return bytes;
        }
    }
    s.as_bytes().to_vec()
}

/// Render stored bytea bytes as PostgREST hex OUTPUT form (`\x<hex>`).
fn bytea_output(bytes: &[u8]) -> String {
    format!("\\x{}", hex::encode(bytes))
}

impl FakePostgrest {
    /// Spawn the fake on an ephemeral loopback port and return its base URL
    /// (`http://127.0.0.1:PORT`). The server lives for the whole test; the
    /// spawned accept loop is detached and dies with the runtime.
    async fn spawn() -> (String, Self) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local_addr");
        let rows: Arc<AsyncMutex<HashMap<String, StoredRow>>> =
            Arc::new(AsyncMutex::new(HashMap::new()));
        let rows_for_loop = rows.clone();

        tokio::spawn(async move {
            loop {
                let (mut sock, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let rows = rows_for_loop.clone();
                tokio::spawn(async move {
                    let _ = handle_conn(&mut sock, &rows).await;
                });
            }
        });

        (format!("http://127.0.0.1:{}", addr.port()), Self { rows })
    }

    /// Directly seed a row as if a cross-client (e.g. Android) writer had
    /// inserted it, using `bytea` INPUT semantics on `payload_ct_str`.
    async fn seed_via_bytea_input(&self, id: &str, item_id: &str, payload_ct_str: &str) {
        self.rows.lock().await.insert(
            id.to_owned(),
            StoredRow {
                item_id: item_id.to_owned(),
                content_type: "text".to_owned(),
                payload_ct_bytes: bytea_input(payload_ct_str),
                lamport_ts: 1,
                wall_time: 1,
                device_id: "device-cross-client".to_owned(),
            },
        );
    }
}

/// Read a full HTTP/1.1 request (headers + Content-Length body) from `sock`,
/// dispatch POST/GET against the row store, and write a PostgREST-shaped
/// response. Deliberately tiny: handles only what these tests exercise.
async fn handle_conn(
    sock: &mut tokio::net::TcpStream,
    rows: &Arc<AsyncMutex<HashMap<String, StoredRow>>>,
) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    // Read until we have headers + the declared Content-Length body.
    loop {
        let n = sock.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(hdr_end) = find_header_end(&buf) {
            let head = String::from_utf8_lossy(&buf[..hdr_end]);
            let content_len = head
                .lines()
                .find_map(|l| {
                    let l = l.to_ascii_lowercase();
                    l.strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if buf.len() >= hdr_end + content_len {
                break;
            }
        }
    }

    let hdr_end = find_header_end(&buf).unwrap_or(buf.len());
    let head = String::from_utf8_lossy(&buf[..hdr_end]).to_string();
    let body = buf[hdr_end..].to_vec();
    let request_line = head.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();

    let response = match method {
        "POST" if target.starts_with("/rest/v1/clipboard_items") => {
            let json: serde_json::Value =
                serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
            // PostgREST accepts a single object or an array of objects.
            let objs: Vec<&serde_json::Value> = match &json {
                serde_json::Value::Array(a) => a.iter().collect(),
                serde_json::Value::Object(_) => vec![&json],
                _ => vec![],
            };
            {
                let mut store = rows.lock().await;
                for obj in objs {
                    let id = obj["id"].as_str().unwrap_or_default().to_owned();
                    let payload_ct_str = obj["payload_ct"].as_str().unwrap_or_default();
                    store.insert(
                        id,
                        StoredRow {
                            item_id: obj["item_id"].as_str().unwrap_or_default().to_owned(),
                            content_type: obj["content_type"].as_str().unwrap_or("text").to_owned(),
                            // bytea INPUT semantics: `\x<hex>` decodes, else
                            // stores the literal ASCII bytes (regression model).
                            payload_ct_bytes: bytea_input(payload_ct_str),
                            lamport_ts: obj["lamport_ts"].as_i64().unwrap_or(0),
                            wall_time: obj["wall_time"].as_i64().unwrap_or(0),
                            device_id: obj["device_id"].as_str().unwrap_or_default().to_owned(),
                        },
                    );
                }
            }
            http_response(201, "")
        }
        "GET" if target.starts_with("/rest/v1/clipboard_items") => {
            let store = rows.lock().await;
            let mut out: Vec<serde_json::Value> = store
                .iter()
                .map(|(id, r)| {
                    serde_json::json!({
                        "id": id,
                        "item_id": r.item_id,
                        "content_type": r.content_type,
                        // bytea OUTPUT form: ALWAYS `\x<hex>`, regardless of
                        // how the value was written. This is the crucial
                        // property the old mocks lacked.
                        "payload_ct": bytea_output(&r.payload_ct_bytes),
                        "lamport_ts": r.lamport_ts,
                        "wall_time": r.wall_time,
                        "expires_at": serde_json::Value::Null,
                        "app_bundle_id": serde_json::Value::Null,
                        "device_id": r.device_id,
                    })
                })
                .collect();
            out.sort_by(|a, b| b["wall_time"].as_i64().cmp(&a["wall_time"].as_i64()));
            http_response(200, &serde_json::to_string(&out).unwrap())
        }
        _ => http_response(404, "[]"),
    };

    sock.write_all(response.as_bytes()).await?;
    sock.flush().await?;
    Ok(())
}

/// Find the byte offset just past the `\r\n\r\n` header terminator.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        404 => "Not Found",
        _ => "Unknown",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn cfg_for(url: &str) -> CloudConfig {
    // Struct literal bypasses `CloudConfig::new`'s HTTPS gate; the loopback
    // http:// URL is permitted at the `start_cloud` gate only under
    // `#[cfg(test)]`. We drive the inner functions directly here.
    CloudConfig {
        supabase_url: url.to_owned(),
        anon_key: "anon-key-for-tests".to_owned(),
        email: None,
        password: None,
    }
}

fn unique_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Minimal `ClipboardItem` for the push path. Only `id`/`item_id` and the
/// serialised JSON columns matter — the payload is carried out-of-band as
/// the pre-encoded `payload_ct_b64` argument to `push_item_with_retries`.
fn make_item(id: &str, item_id: &str) -> ClipboardItem {
    ClipboardItem {
        deleted: false,
        id: id.to_owned(),
        item_id: item_id.to_owned(),
        content_type: "text".to_owned(),
        content: Some(b"local-ct".to_vec()),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: 1,
        wall_time: 1,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: String::new(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

/// **(a) Daemon push round-trip through the HTTP layer.**
///
/// encrypt → `encode_payload_ct_hex` → POST (real `push_item_with_retries`)
/// → GET (real `fetch_remote_rows`) → `decode_payload_ct` → `decrypt_from_cloud`
/// recovers the original plaintext. Also asserts the value the daemon sends
/// over the wire begins with `\x` and is valid lower-hex.
#[tokio::test]
async fn daemon_push_roundtrips_through_bytea_wire() {
    let (url, server) = FakePostgrest::spawn().await;
    let client = reqwest::Client::new();
    let cfg = cfg_for(&url);
    let bearer = Arc::new(RwLock::new("anon-key-for-tests".to_owned()));

    let sync_key = derive_sync_key("daemon-push-passphrase").expect("derive sync key");
    let id = unique_id();
    let item_id = unique_id();
    let plaintext = b"daemon push -> bytea wire -> back";

    let blob = encrypt_for_cloud(&sync_key, &item_id, plaintext).expect("cloud encrypt");
    let payload_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);

    // Assert the WIRE form the daemon serialises is the bytea hex literal.
    let wire = encode_payload_ct_hex(&payload_ct_b64);
    assert!(
        wire.starts_with("\\x"),
        "daemon must send payload_ct as a bytea hex literal, got: {wire:?}"
    );
    assert!(
        hex::decode(&wire[2..]).is_ok(),
        "the bytes after \\x must be valid hex"
    );

    let item = make_item(&id, &item_id);
    let rest_url = format!("{url}/rest/v1/clipboard_items");
    // Session-less auth client: the fake never returns 401, so the refresh
    // path is not exercised; we just satisfy the merged signature.
    let auth = AuthClient::new(cfg.supabase_url.clone(), cfg.anon_key.clone());
    push_item_with_retries(
        &client,
        &rest_url,
        &cfg,
        &bearer,
        &item,
        Some(payload_ct_b64.as_str()),
        None,
        &auth,
    )
    .await
    .expect("push must land in the fake PostgREST");

    // The server stored the DECODED ciphertext bytes (not the ASCII of the
    // hex literal), proving `encode_payload_ct_hex` was interpreted as bytea.
    {
        let stored = server.rows.lock().await;
        let row = stored.get(&id).expect("row present after push");
        assert_eq!(
            row.payload_ct_bytes, blob,
            "server must hold the true ciphertext bytes, not the hex ASCII"
        );
    }

    // Poll it back through the real GET path and the product decoder.
    let poll_url = format!(
        "{url}/rest/v1/clipboard_items?select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc&limit=20"
    );
    let rows =
        match fetch_remote_rows(&client, &poll_url, &cfg.anon_key, "anon-key-for-tests").await {
            FetchOutcome::Ok(rows) => rows,
            FetchOutcome::Unauthorized => panic!("fetch_remote_rows: 401 Unauthorized"),
            FetchOutcome::RateLimited(d) => {
                panic!("fetch_remote_rows: 429 rate-limited (Retry-After: {d:?})")
            }
            FetchOutcome::Failed(e) => panic!("fetch_remote_rows failed: {e}"),
        };
    let row = rows
        .iter()
        .find(|r| r["id"].as_str() == Some(id.as_str()))
        .expect("pushed row must come back from GET");
    let returned = row["payload_ct"].as_str().expect("payload_ct string");
    assert!(
        returned.starts_with("\\x"),
        "PostgREST returns bytea in hex OUTPUT form; got {returned:?}"
    );
    let decoded = decode_payload_ct(returned).expect("decode_payload_ct");
    let recovered = decrypt_from_cloud(&sync_key, &item_id, &decoded).expect("decrypt round-trip");
    assert_eq!(recovered, plaintext, "round-trip plaintext mismatch");
}

/// **(b) Cross-client contract — the regression-catching test.**
///
/// Positive: a correctly-written cross-client row (raw ciphertext bytes,
/// returned as `\x<hex>`) decrypts. Negative: the OLD BROKEN Android form
/// (BARE BASE64 text stored verbatim, then returned as `\x<hex-of-base64-
/// ASCII>`) must FAIL to decrypt — encoding the contract so the regression
/// can never silently come back.
#[tokio::test]
async fn cross_client_contract_correct_decrypts_broken_fails() {
    let (url, server) = FakePostgrest::spawn().await;
    let client = reqwest::Client::new();
    let cfg = cfg_for(&url);

    let sync_key = derive_sync_key("cross-client-passphrase").expect("derive sync key");
    let plaintext = b"cross-client payload from Android";

    // ── Correct cross-client row: stored as a proper bytea hex literal. ──
    let good_id = unique_id();
    let good_item_id = unique_id();
    let blob = encrypt_for_cloud(&sync_key, &good_item_id, plaintext).expect("cloud encrypt");
    let good_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let good_hex_literal = encode_payload_ct_hex(&good_b64); // "\x..."
    server
        .seed_via_bytea_input(&good_id, &good_item_id, &good_hex_literal)
        .await;

    // ── Broken (Android regression) row: bare BASE64 stored verbatim. The
    //    fake stores its literal ASCII bytes (Postgres bytea input on a
    //    non-`\x` string), then renders `\x<hex-of-those-ASCII-bytes>`. ──
    let bad_id = unique_id();
    let bad_item_id = unique_id();
    let bad_blob = encrypt_for_cloud(&sync_key, &bad_item_id, plaintext).expect("cloud encrypt");
    let bad_b64 = base64::engine::general_purpose::STANDARD.encode(&bad_blob);
    // NOTE: bare base64, NOT run through encode_payload_ct_hex.
    server
        .seed_via_bytea_input(&bad_id, &bad_item_id, &bad_b64)
        .await;

    let poll_url = format!(
        "{url}/rest/v1/clipboard_items?select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc&limit=20"
    );
    let rows =
        match fetch_remote_rows(&client, &poll_url, &cfg.anon_key, "anon-key-for-tests").await {
            FetchOutcome::Ok(rows) => rows,
            FetchOutcome::Unauthorized => panic!("fetch: 401 Unauthorized"),
            FetchOutcome::RateLimited(d) => panic!("fetch: 429 rate-limited (Retry-After: {d:?})"),
            FetchOutcome::Failed(e) => panic!("fetch failed: {e}"),
        };

    let good_row = rows
        .iter()
        .find(|r| r["id"].as_str() == Some(good_id.as_str()))
        .expect("good row present");
    let bad_row = rows
        .iter()
        .find(|r| r["id"].as_str() == Some(bad_id.as_str()))
        .expect("bad row present");

    // Both are served in hex OUTPUT form by the bytea-faithful fake.
    let good_pc = good_row["payload_ct"].as_str().unwrap();
    let bad_pc = bad_row["payload_ct"].as_str().unwrap();
    assert!(good_pc.starts_with("\\x") && bad_pc.starts_with("\\x"));

    // POSITIVE: correct cross-client encoding round-trips.
    let good_decoded = decode_payload_ct(good_pc).expect("decode good");
    let good_plain =
        decrypt_from_cloud(&sync_key, &good_item_id, &good_decoded).expect("good decrypt");
    assert_eq!(
        good_plain, plaintext,
        "correct cross-client form must decrypt"
    );

    // NEGATIVE (TEETH): the broken bare-base64 form must NOT decrypt. The
    // decoded `\x<hex>` here is the ASCII of the base64 string, i.e. the
    // wrong bytes, so the AEAD tag check rejects it.
    let bad_decoded = decode_payload_ct(bad_pc).expect("decode bad (hex itself is valid)");
    assert_ne!(
        bad_decoded, bad_blob,
        "regression model: stored bytes must be the base64 ASCII, not the ciphertext"
    );
    let bad_result = decrypt_from_cloud(&sync_key, &bad_item_id, &bad_decoded);
    assert!(
        bad_result.is_err(),
        "TEETH: the old bare-base64 Android form MUST fail to decrypt; \
         if this ever passes, the cross-platform payload_ct bug has regressed"
    );
}

/// **(c) Drive the poll-path HTTP layer with refresh.**
///
/// Exercises `fetch_remote_rows_with_refresh` (the function the realtime
/// loop actually calls) against the fake, proving the encode/decode+decrypt
/// round-trip works through the same helper the daemon uses on every tick.
#[tokio::test]
async fn poll_path_with_refresh_roundtrips() {
    let (url, server) = FakePostgrest::spawn().await;
    let client = reqwest::Client::new();
    let cfg = cfg_for(&url);
    let bearer = Arc::new(RwLock::new("anon-key-for-tests".to_owned()));

    let sync_key = derive_sync_key("poll-path-passphrase").expect("derive sync key");
    let id = unique_id();
    let item_id = unique_id();
    let plaintext = b"poll-path payload through fetch_remote_rows_with_refresh";

    let blob = encrypt_for_cloud(&sync_key, &item_id, plaintext).expect("cloud encrypt");
    let b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    server
        .seed_via_bytea_input(&id, &item_id, &encode_payload_ct_hex(&b64))
        .await;

    let poll_url = format!(
        "{url}/rest/v1/clipboard_items?select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc&limit=20"
    );
    let signed_in = Arc::new(std::sync::atomic::AtomicBool::new(true));
    // Session-less auth client: this fake never returns 401, so the refresh
    // path is not exercised here; we just need a value for the merged signature.
    let auth = AuthClient::new(cfg.supabase_url.clone(), cfg.anon_key.clone());
    let rows = fetch_remote_rows_with_refresh(&client, &poll_url, &cfg, &bearer, &signed_in, &auth)
        .await
        .expect("poll-path fetch must succeed");
    let row = rows
        .iter()
        .find(|r| r["id"].as_str() == Some(id.as_str()))
        .expect("seeded row must come back");
    let decoded = decode_payload_ct(row["payload_ct"].as_str().unwrap()).expect("decode");
    let recovered = decrypt_from_cloud(&sync_key, &item_id, &decoded).expect("decrypt");
    assert_eq!(recovered, plaintext, "poll-path round-trip mismatch");
}

// ── BUG C1: cloud file-identity envelope ──────────────────────────────────

/// Upload-encode → download-decode preserves the file name and MIME embedded
/// in the encrypted plaintext (the Supabase schema carries neither).
#[test]
fn cloud_file_header_round_trips_name_and_mime() {
    let name = "Q1 report (final).pdf";
    let mime = "application/pdf";
    let file_bytes = b"%PDF-1.7\n...binary file contents...\x00\xff".to_vec();

    let wrapped = encode_cloud_file_payload(name, mime, &file_bytes);
    // Header must actually prepend bytes (version + 2 len fields + strings).
    assert!(wrapped.len() > file_bytes.len());
    assert_eq!(wrapped[0], CLOUD_FILE_HEADER_VERSION);

    let (recovered_bytes, recovered_name, recovered_mime) = decode_cloud_file_payload(&wrapped);
    assert_eq!(recovered_bytes, file_bytes, "file bytes must survive");
    assert_eq!(recovered_name, name, "file name must survive");
    assert_eq!(recovered_mime, mime, "mime must survive");
}

/// A non-ASCII (UTF-8) file name round-trips intact through the header.
#[test]
fn cloud_file_header_handles_utf8_name() {
    let name = "résumé — 履歴書.txt";
    let mime = "text/plain";
    let file_bytes = b"hello".to_vec();
    let wrapped = encode_cloud_file_payload(name, mime, &file_bytes);
    let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
    assert_eq!(rb, file_bytes);
    assert_eq!(rn, name);
    assert_eq!(rm, mime);
}

/// BUG C1 back-compat: a payload uploaded by an OLD daemon has no header.
/// It must decode as raw file bytes with the legacy name/MIME, never panic.
#[test]
fn cloud_file_legacy_headerless_payload_decodes_as_raw() {
    // Bytes whose first byte is NOT the header version → treated as raw.
    let raw = b"\x99 arbitrary legacy file bytes with no envelope".to_vec();
    let (bytes, name, mime) = decode_cloud_file_payload(&raw);
    assert_eq!(bytes, raw, "entire buffer is the file");
    assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
    assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);
}

/// A payload that starts with the version byte but whose length fields
/// overrun the buffer is treated as legacy raw bytes, not parsed past the
/// end (no panic).
#[test]
fn cloud_file_malformed_header_falls_back_to_legacy() {
    // version=1, name_len declares 0xFFFF bytes but none follow.
    let malformed = vec![CLOUD_FILE_HEADER_VERSION, 0xFF, 0xFF, 0x00];
    let (bytes, name, mime) = decode_cloud_file_payload(&malformed);
    assert_eq!(bytes, malformed);
    assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
    assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);

    // Too short to even hold the minimal 5-byte header.
    let tiny = vec![CLOUD_FILE_HEADER_VERSION, 0x00];
    let (b2, n2, _) = decode_cloud_file_payload(&tiny);
    assert_eq!(b2, tiny);
    assert_eq!(n2, CLOUD_FILE_LEGACY_NAME);
}

/// Empty name/mime (zero-length fields) form a valid header and round-trip
/// to empty strings — the smallest legal envelope.
#[test]
fn cloud_file_empty_fields_form_valid_header() {
    let file_bytes = b"x".to_vec();
    let wrapped = encode_cloud_file_payload("", "", &file_bytes);
    assert_eq!(wrapped.len(), 5 + file_bytes.len());
    let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
    assert_eq!(rb, file_bytes);
    assert_eq!(rn, "");
    assert_eq!(rm, "");
}

// ── Coherence fix: upload ceiling checks the WRAPPED quantity ──────────────

/// A minimal `content_type == "file"` item with a valid `blob_ref` meta so
/// `wrap_cloud_upload_plaintext` can read its name/MIME.
fn file_item(id: &str, name: &str, mime: &str, original_size: usize) -> ClipboardItem {
    ClipboardItem {
        deleted: false,
        id: id.to_owned(),
        item_id: id.to_owned(),
        content_type: "file".to_owned(),
        content: Some(Vec::new()),
        content_nonce: None,
        blob_ref: Some(
            serde_json::json!({
                "filename": name,
                "mime": mime,
                "original_size": original_size,
                "chunk_count": 1,
                "file_id": vec![0u8; 16],
            })
            .to_string(),
        ),
        is_sensitive: false,
        is_synced: false,
        lamport_ts: 1,
        wall_time: 1,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: String::new(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

/// A file whose RAW plaintext fits under the sync ceiling but whose WRAPPED
/// (header-prepended) payload exceeds it must be SKIPPED on upload — exactly
/// what `build_local_blob_item` would reject on download. This asserts the two
/// ends now check the same quantity, closing the one-sided-failure window.
#[test]
fn cloud_upload_skips_file_whose_wrapped_payload_exceeds_ceiling() {
    let ceiling = crate::sync_orch::SYNC_MAX_BLOB_BYTES;
    let name = "huge.bin";
    let mime = "application/octet-stream";
    // Header overhead = 1 (version) + 2 + name.len() + 2 + mime.len().
    let header_overhead = 1 + 2 + name.len() + 2 + mime.len();

    // RAW plaintext is exactly the ceiling → would PASS a raw-only check, but
    // once the header is prepended the wrapped buffer is `header_overhead`
    // bytes over the ceiling.
    let raw = vec![0u8; ceiling];

    let item = file_item("file-1", name, mime, raw.len());

    let err = wrap_and_check_cloud_upload_plaintext(&item, raw)
        .expect_err("wrapped payload over the ceiling must be skipped, not uploaded");
    assert!(
        err.contains("exceeds cloud sync ceiling"),
        "unexpected error message: {err}"
    );
    // Sanity: the rejected size is the wrapped size, not the raw size.
    let expected = ceiling + header_overhead;
    assert!(
        err.contains(&expected.to_string()),
        "error should report the WRAPPED size {expected}: {err}"
    );
}

/// The boundary: a file whose WRAPPED payload is exactly the ceiling is
/// accepted (upload and download agree on `<=` vs `>`).
#[test]
fn cloud_upload_accepts_file_whose_wrapped_payload_equals_ceiling() {
    let ceiling = crate::sync_orch::SYNC_MAX_BLOB_BYTES;
    let name = "ok.bin";
    let mime = "application/octet-stream";
    let header_overhead = 1 + 2 + name.len() + 2 + mime.len();
    let raw = vec![7u8; ceiling - header_overhead];

    let item = file_item("file-2", name, mime, raw.len());

    let wrapped = wrap_and_check_cloud_upload_plaintext(&item, raw)
        .expect("a wrapped payload exactly at the ceiling must be accepted");
    assert_eq!(
        wrapped.len(),
        ceiling,
        "wrapped size should hit the ceiling exactly"
    );
}
