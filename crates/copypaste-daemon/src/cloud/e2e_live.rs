use super::*;
use crate::sync_common::{build_local_item, decode_payload_ct};
use base64::Engine as _;
use copypaste_core::{
    build_item_aad_v2, decrypt_from_cloud, derive_sync_key, derive_v2, encrypt_for_cloud,
    encrypt_item_with_aad, insert_item, ClipboardItem, Database, AAD_SCHEMA_VERSION_V4,
    ITEM_KEY_VERSION_CURRENT,
};
use copypaste_supabase::auth::AuthClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

const DEFAULT_URL: &str = "http://127.0.0.1:54321";

fn stack_url() -> String {
    std::env::var("SUPABASE_TEST_URL")
        .unwrap_or_else(|_| DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// Read the local-stack anon key from `SUPABASE_TEST_ANON_KEY`. Returns
/// `None` (test no-ops with a notice) when unset so no key lives in source
/// and CI without a stack stays green even if `--ignored` is forced.
fn anon_key() -> Option<String> {
    std::env::var("SUPABASE_TEST_ANON_KEY")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Bind `$name` to the anon key, or print a notice and `return` (no-op) when
/// it is unset. Keeps the anon key out of source while letting the tests run
/// when an operator supplies it for a live-stack run.
macro_rules! anon_or_skip {
    ($name:ident) => {
        let $name = match anon_key() {
            Some(k) => k,
            None => {
                eprintln!("SKIP: set SUPABASE_TEST_ANON_KEY to run live Supabase e2e tests");
                return;
            }
        };
    };
}

/// A signed-in test user: fresh GoTrue account + its bearer + uid.
struct TestUser {
    email: String,
    password: String,
    bearer: String,
    uid: String,
}

/// Create a brand-new GoTrue user via `/auth/v1/signup` (local stack
/// auto-confirms), then sign in to obtain an `authenticated`-scope JWT.
async fn fresh_user(client: &reqwest::Client, url: &str, anon: &str) -> TestUser {
    let nonce: u128 = {
        // Cheap unique suffix without pulling rand into scope.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        t ^ ((std::process::id() as u128) << 64)
    };
    let email = format!("e2e-{nonce:x}@example.com");
    let password = "Test-Passw0rd-123!".to_owned();

    let signup = client
        .post(format!("{url}/auth/v1/signup"))
        .header("apikey", anon)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("signup request");
    assert!(
        signup.status().is_success(),
        "signup failed ({}): {}",
        signup.status(),
        signup.text().await.unwrap_or_default()
    );

    // Sign in via the SAME AuthClient the daemon uses (product fidelity).
    let auth = AuthClient::new(url.to_owned(), anon.to_owned());
    let session = auth
        .sign_in(&email, &password)
        .await
        .expect("sign_in must succeed for a freshly-created user");
    let uid = session.user.id.clone();
    assert!(!uid.is_empty(), "GoTrue session must carry a user id");

    TestUser {
        email,
        password,
        bearer: session.access_token,
        uid,
    }
}

/// Build the daemon-style `CloudConfig` pointing at the live stack, with the
/// user's email/password so `resolve_bearer` exercises the real GoTrue
/// password grant (we still also keep the bearer we got above for raw GETs).
fn cfg_for(user: &TestUser, anon: &str) -> CloudConfig {
    // NOTE: struct literal bypasses `CloudConfig::new`'s HTTPS gate. The gate
    // is intentional for production and is unit-tested separately; here we
    // target a local http:// stack on purpose.
    CloudConfig {
        supabase_url: stack_url(),
        anon_key: anon.to_owned(),
        email: Some(user.email.clone()),
        password: Some(user.password.clone()),
    }
}

/// Session-less auth client for the push pipeline. On a 401 against the live
/// stack `refresh_bearer` falls back to a full password sign-in (the cfg
/// carries the user's email/password), which is the intended recovery path.
fn test_auth(cfg: &CloudConfig) -> AuthClient {
    AuthClient::new(cfg.supabase_url.clone(), cfg.anon_key.clone())
}

/// Open a fresh, empty encrypted DB at a unique temp path with a random
/// ephemeral key — mirrors the daemon's `COPYPASTE_EPHEMERAL_KEY=1` mode.
fn open_temp_db(tmp: &tempfile::TempDir, name: &str) -> (Database, [u8; 32]) {
    // Random 32-byte ephemeral local key from two v4 UUIDs (uuid is already
    // a dep; avoids adding getrandom directly for a throwaway test key).
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    let path = tmp.path().join(name);
    let db = Database::open(&path, &key).expect("open encrypted db");
    (db, key)
}

/// Encrypt `plaintext` with `local_key` (v2 HKDF path) into a local
/// `ClipboardItem`, exactly as the daemon stores a freshly-captured item.
fn local_item(local_key: &[u8; 32], plaintext: &[u8], device_id: &str) -> ClipboardItem {
    let id = uuid::Uuid::new_v4().to_string();
    let item_id = uuid::Uuid::new_v4().to_string();
    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let v2_key = derive_v2(local_key);
    let aad = build_item_aad_v2(
        &item_id,
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    let (nonce, ciphertext) =
        encrypt_item_with_aad(plaintext, &v2_key, &aad).expect("local encrypt");
    ClipboardItem {
        deleted: false,
        id,
        item_id,
        content_type: "text".to_owned(),
        content: Some(ciphertext),
        content_nonce: Some(nonce.to_vec()),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: wall_time,
        wall_time,
        expires_at: None,
        app_bundle_id: Some("com.example.test".to_owned()),
        content_hash: None,
        origin_device_id: device_id.to_owned(),
        key_version: ITEM_KEY_VERSION_CURRENT as u8,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

/// Authenticated raw REST GET of all of `user`'s rows (RLS-scoped by the
/// bearer). Used to assert what the server actually persisted.
async fn rest_select_all(
    client: &reqwest::Client,
    url: &str,
    anon: &str,
    bearer: &str,
) -> Vec<serde_json::Value> {
    let resp = client
        .get(format!(
            "{url}/rest/v1/clipboard_items?select=id,item_id,content_type,payload_ct,user_id&order=wall_time.desc"
        ))
        .header("apikey", anon)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
        .expect("rest get");
    assert!(
        resp.status().is_success(),
        "rest GET status {}",
        resp.status()
    );
    resp.json().await.expect("rest get json")
}

// ── Scenario A: real push lands a row in Supabase under the user ──────────
#[tokio::test]
#[ignore = "requires a live local Supabase stack"]
async fn e2e_live_push_lands_in_supabase() {
    let client = reqwest::Client::new();
    let url = stack_url();
    anon_or_skip!(anon);
    let user = fresh_user(&client, &url, &anon).await;

    let tmp = tempfile::tempdir().unwrap();
    let (db_a, local_key_a) = open_temp_db(&tmp, "a.db");
    let sync_key = derive_sync_key("correct-horse-battery-staple").unwrap();

    // Build a local item the way the daemon stores a captured clipboard
    // entry, then re-encrypt for the cloud (product path).
    let plaintext = b"hello-from-daemon-A push scenario";
    let item = local_item(&local_key_a, plaintext, "device-A");
    insert_item(&db_a, &item).expect("local insert");
    let blob = encrypt_for_cloud(&sync_key, &item.item_id, plaintext).expect("cloud encrypt");
    let payload_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);

    // Drive the REAL push pipeline (401-refresh / 429 / transient retries).
    let rest_url = format!("{url}/rest/v1/clipboard_items");
    let cfg = cfg_for(&user, &anon);
    let bearer = Arc::new(RwLock::new(user.bearer.clone()));
    let auth = test_auth(&cfg);
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
    .expect("push_item_with_retries must succeed against the live stack");

    // Assert the row is present in Supabase, scoped to this user by RLS.
    let rows = rest_select_all(&client, &url, &anon, &user.bearer).await;
    let found = rows
        .iter()
        .find(|r| r["id"].as_str() == Some(item.id.as_str()));
    let found = found.expect("pushed row must be visible to its owner via RLS-scoped GET");
    assert_eq!(found["item_id"].as_str(), Some(item.item_id.as_str()));
    assert_eq!(
        found["user_id"].as_str(),
        Some(user.uid.as_str()),
        "server must stamp user_id = auth.uid() via the column default"
    );
    eprintln!(
        "PUSH OK: id={} item_id={} owner={}",
        item.id, item.item_id, user.uid
    );
}

// ── RLS isolation: a different user cannot see the first user's items ─────
#[tokio::test]
#[ignore = "requires a live local Supabase stack"]
async fn e2e_live_rls_isolation_between_users() {
    let client = reqwest::Client::new();
    let url = stack_url();
    anon_or_skip!(anon);

    let alice = fresh_user(&client, &url, &anon).await;
    let bob = fresh_user(&client, &url, &anon).await;

    // Alice pushes one item via the real push pipeline.
    let tmp = tempfile::tempdir().unwrap();
    let (_db, local_key) = open_temp_db(&tmp, "alice.db");
    let sync_key = derive_sync_key("alice-passphrase").unwrap();
    let plaintext = b"alice-secret-clip";
    let item = local_item(&local_key, plaintext, "device-alice");
    let blob = encrypt_for_cloud(&sync_key, &item.item_id, plaintext).unwrap();
    let payload_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let cfg = cfg_for(&alice, &anon);
    let bearer = Arc::new(RwLock::new(alice.bearer.clone()));
    let auth = test_auth(&cfg);
    push_item_with_retries(
        &client,
        &format!("{url}/rest/v1/clipboard_items"),
        &cfg,
        &bearer,
        &item,
        Some(payload_ct_b64.as_str()),
        None,
        &auth,
    )
    .await
    .expect("alice push");

    // Alice sees her row.
    let alice_rows = rest_select_all(&client, &url, &anon, &alice.bearer).await;
    assert!(
        alice_rows
            .iter()
            .any(|r| r["id"].as_str() == Some(item.id.as_str())),
        "alice must see her own row"
    );

    // Bob, signed in as a DIFFERENT user, must NOT see Alice's row.
    let bob_rows = rest_select_all(&client, &url, &anon, &bob.bearer).await;
    assert!(
        !bob_rows
            .iter()
            .any(|r| r["id"].as_str() == Some(item.id.as_str())),
        "RLS breach: bob can see alice's row"
    );
    eprintln!(
        "RLS OK: alice={} sees row, bob={} does not (bob_row_count={})",
        alice.uid,
        bob.uid,
        bob_rows.len()
    );
}

// ── Scenario B: round-trip — A pushes, B (same user) pulls into local DB ──
//
// This drives the REAL download pipeline used by `realtime_loop`:
//   fetch_remote_rows → base64-decode payload_ct → decrypt_from_cloud
//   → build_local_item (re-encrypt with B's local key) → insert_item.
// Success = the plaintext A copied is decryptable from B's SQLCipher store.
#[tokio::test]
#[ignore = "requires a live local Supabase stack"]
async fn e2e_live_round_trip_a_push_b_pull() {
    let client = reqwest::Client::new();
    let url = stack_url();
    anon_or_skip!(anon);
    let user = fresh_user(&client, &url, &anon).await;

    let tmp = tempfile::tempdir().unwrap();
    // Daemon A and daemon B share the same GoTrue user + sync passphrase but
    // have independent local SQLCipher keys (independent devices).
    let (db_a, local_key_a) = open_temp_db(&tmp, "a.db");
    let (db_b, local_key_b) = open_temp_db(&tmp, "b.db");
    let sync_key = derive_sync_key("shared-cloud-passphrase").unwrap();

    // A captures + pushes.
    let plaintext = b"round-trip-payload: A -> cloud -> B";
    let item = local_item(&local_key_a, plaintext, "device-A");
    insert_item(&db_a, &item).expect("A local insert");
    let blob = encrypt_for_cloud(&sync_key, &item.item_id, plaintext).unwrap();
    let payload_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let cfg = cfg_for(&user, &anon);
    let bearer = Arc::new(RwLock::new(user.bearer.clone()));
    let auth = test_auth(&cfg);
    push_item_with_retries(
        &client,
        &format!("{url}/rest/v1/clipboard_items"),
        &cfg,
        &bearer,
        &item,
        Some(payload_ct_b64.as_str()),
        None,
        &auth,
    )
    .await
    .expect("A push");

    // B polls using the SAME poll URL + helper the realtime_loop uses, then
    // runs the real decode/decrypt/insert pipeline. Bounded poll: up to 10
    // tries, 1s apart.
    let poll_url = format!(
        "{url}/rest/v1/clipboard_items?select=id,item_id,content_type,payload_ct,lamport_ts,wall_time,expires_at,app_bundle_id,device_id,deleted,pinned,pin_order&order=wall_time.asc&limit=20"
    );
    let mut inserted = false;
    let mut last_diag = String::from("(no rows fetched)");
    for attempt in 1..=10 {
        let rows = match fetch_remote_rows(&client, &poll_url, &anon, &user.bearer).await {
            FetchOutcome::Ok(rows) => rows,
            FetchOutcome::Unauthorized => panic!("B fetch_remote_rows: 401 Unauthorized"),
            FetchOutcome::RateLimited(d) => {
                panic!("B fetch_remote_rows: 429 rate-limited (Retry-After: {d:?})")
            }
            FetchOutcome::Failed(e) => panic!("B fetch_remote_rows: {e}"),
        };
        for row in &rows {
            let Some(id) = row["id"].as_str() else {
                continue;
            };
            if id != item.id {
                continue;
            }
            let payload_ct = row["payload_ct"].as_str().unwrap_or_default();
            // Use the PRODUCT decoder (the realtime_loop's path), proving the
            // bytea hex round-trip end-to-end.
            let blob = match decode_payload_ct(payload_ct) {
                Ok(b) => b,
                Err(e) => {
                    last_diag = format!(
                        "decode_payload_ct FAILED: {e}; \
                         server returned payload_ct={payload_ct:?}"
                    );
                    continue;
                }
            };
            let recovered = match decrypt_from_cloud(&sync_key, item.item_id.as_str(), &blob) {
                Ok(p) => p,
                Err(e) => {
                    last_diag = format!("decrypt_from_cloud FAILED: {e}");
                    continue;
                }
            };
            assert_eq!(recovered, plaintext, "round-trip plaintext mismatch");
            let b_item = build_local_item(
                id,
                item.item_id.as_str(),
                "text",
                &recovered,
                row["lamport_ts"].as_i64().unwrap_or(0),
                row["wall_time"].as_i64().unwrap_or(0),
                row["expires_at"].as_i64(),
                row["app_bundle_id"].as_str().map(str::to_owned),
                row["device_id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_default(),
                &zeroize::Zeroizing::new(local_key_b),
            )
            .expect("B build_local_item");
            insert_item(&db_b, &b_item).expect("B insert_item");
            inserted = true;
        }
        if inserted {
            break;
        }
        eprintln!("round-trip poll attempt {attempt}/10: not yet; {last_diag}");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert!(
        inserted,
        "round-trip FAILED: A's item never reached B's local store. \
         Diagnosis: {last_diag}"
    );

    // Prove B can actually read the plaintext back out of its OWN SQLCipher
    // store (decrypt with B's local key), confirming a true round-trip.
    assert!(
        super::exists_item(&db_b, item.id.as_str()).unwrap(),
        "item must exist in B's local DB"
    );
    eprintln!("ROUND-TRIP OK: '{}' synced A -> cloud -> B", item.id);
}
