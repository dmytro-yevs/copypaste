//! Proof-of-possession (PoP) verification tests for the relay registration endpoint.
//!
//! Security bug CopyPaste-n2l: `register_device` previously accepted ANY X25519
//! public key with NO proof that the registrant holds the corresponding private
//! key. A network attacker could register using a victim's `device_id` and then
//! receive that victim's encrypted inbox items.
//!
//! Fix: the registrant must present a `pop_b64` field —
//! `HMAC-SHA256(key=sync_key, msg="relay-registration-pop-v1:" + device_id)` —
//! to prove it holds the sync key that the `device_id` was derived from.
//!
//! These tests are written FIRST (RED) and must fail before the fix is applied.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/quota.rs"]
mod quota;
#[path = "../src/routes/devices.rs"]
mod routes_devices;
#[path = "../src/routes/health.rs"]
mod routes_health;
#[path = "../src/routes/items.rs"]
mod routes_items;
#[path = "../src/state/mod.rs"]
mod state;

use config::RelayConfig;
use state::{AppState, RelayStore};

// ---------------------------------------------------------------------------
// PoP helper — mirrors what the daemon computes
// ---------------------------------------------------------------------------

/// Compute HMAC-SHA256(key=sync_key, msg="relay-registration-pop-v1:" + device_id)
/// and return it as base64-standard-encoded bytes.
///
/// This mirrors `copypaste_core::derive_relay_registration_pop` so the test
/// helper does not need to link the core crate (the test binary uses path
/// includes, not extern crate).
fn compute_pop(sync_key: &[u8; 32], device_id: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let prefix = b"relay-registration-pop-v1:";
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(sync_key).expect("HMAC accepts any key");
    mac.update(prefix);
    mac.update(device_id.as_bytes());
    let out = mac.finalize().into_bytes();
    B64.encode(out)
}

// ---------------------------------------------------------------------------
// Router helper — mirrors the integration test setup
// ---------------------------------------------------------------------------

fn relay_router(app_state: AppState, config: RelayConfig) -> axum::Router {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/health", get(routes_health::handle))
        .route("/devices", post(routes_devices::register))
        .route("/devices/{device_id}", get(routes_devices::get_device))
        .route(
            "/devices/{device_id}/items",
            get(routes_items::pull).post(routes_items::push),
        )
        .with_state(app_state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
        .layer(axum::Extension(config))
}

fn make_app() -> (axum::Router, AppState) {
    let config = RelayConfig::default();
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);
    (router, app_state)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A fixed 32-byte sync key used as the "legitimate account's" secret.
const SYNC_KEY: [u8; 32] = [0xAA; 32];
/// A different 32-byte sync key simulating an attacker who does NOT share the account.
const ATTACKER_KEY: [u8; 32] = [0xBB; 32];

/// The shared device_id / inbox id (a valid UUID v4-shaped string).
const DEVICE_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

/// A 32-byte public key (HKDF of SYNC_KEY — but for PoP tests the exact bytes
/// don't matter; what matters is the PoP field).
fn valid_pubkey() -> String {
    B64.encode([0xCC_u8; 32])
}

// ---------------------------------------------------------------------------
// POST /devices helper
// ---------------------------------------------------------------------------

async fn post_register(app: axum::Router, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ---------------------------------------------------------------------------
// Test 1: registration WITHOUT a PoP is rejected (400 Bad Request).
// ---------------------------------------------------------------------------

/// A registration request that omits the `pop_b64` field entirely must be
/// rejected with 400. Before the fix this returns 201 — the test must FAIL.
#[tokio::test]
async fn registration_without_pop_is_rejected() {
    let (app, _state) = make_app();
    let body = json!({
        "device_id": DEVICE_ID,
        "device_name": "Legitimate Device",
        "public_key_b64": valid_pubkey(),
        // pop_b64 intentionally absent
    });
    let (status, resp_body) = post_register(app, body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "registration without pop_b64 must be rejected with 400; got {status} body={resp_body}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: registration WITH a valid PoP is accepted (201 Created).
// ---------------------------------------------------------------------------

/// A registration request that includes the correct
/// `HMAC-SHA256(sync_key, prefix || device_id)` proof must succeed with 201
/// and return an auth token.
#[tokio::test]
async fn registration_with_valid_pop_is_accepted() {
    let (app, _state) = make_app();
    let pop = compute_pop(&SYNC_KEY, DEVICE_ID);
    let body = json!({
        "device_id": DEVICE_ID,
        "device_name": "Legitimate Device",
        "public_key_b64": valid_pubkey(),
        "pop_b64": pop,
    });
    let (status, resp_body) = post_register(app, body).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "registration with valid pop_b64 must succeed with 201; got {status} body={resp_body}"
    );
    assert!(
        resp_body["auth_token"].is_string(),
        "201 response must include an auth_token; got {resp_body}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: co-registration with a WRONG PoP (attacker key) is rejected (400).
//
// Scenario: legitimate device registers first with SYNC_KEY's PoP.
// Attacker who only knows DEVICE_ID (but not SYNC_KEY) attempts to
// co-register with a PoP derived from their own ATTACKER_KEY.
// The relay must reject the co-registration because the PoPs don't match.
// ---------------------------------------------------------------------------

/// An attacker who knows the victim's `device_id` but does NOT hold the
/// victim's sync key must be rejected when attempting to co-register.
/// Before the fix, this returns 201 (the attacker gets a valid token).
/// After the fix it must return 400.
#[tokio::test]
async fn coregistration_with_wrong_pop_is_rejected() {
    let (app, _state) = make_app();

    // Step 1: legitimate device registers successfully.
    let legit_pop = compute_pop(&SYNC_KEY, DEVICE_ID);
    let first_body = json!({
        "device_id": DEVICE_ID,
        "device_name": "Legitimate Device",
        "public_key_b64": valid_pubkey(),
        "pop_b64": legit_pop,
    });
    let (status1, _) = post_register(app.clone(), first_body).await;
    assert_eq!(
        status1,
        StatusCode::CREATED,
        "first (legitimate) registration must succeed"
    );

    // Step 2: attacker attempts to co-register the SAME device_id with a PoP
    // derived from their own different key (they don't know SYNC_KEY).
    let attacker_pop = compute_pop(&ATTACKER_KEY, DEVICE_ID);
    let attacker_body = json!({
        "device_id": DEVICE_ID,
        "device_name": "Attacker Device",
        "public_key_b64": valid_pubkey(),
        "pop_b64": attacker_pop,
    });
    let (status2, resp_body2) = post_register(app, attacker_body).await;
    // CopyPaste-crh3.12: rejected with a GENERIC 401 (not a verbose 400 that
    // would confirm the device_id is already registered — a registration
    // oracle). The body must not reveal registration state.
    assert_eq!(
        status2,
        StatusCode::UNAUTHORIZED,
        "co-registration with wrong pop_b64 must be a generic 401; got {status2} body={resp_body2}"
    );
    assert!(
        !resp_body2.to_string().contains("does not match"),
        "401 body must not leak registration state: {resp_body2}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: co-registration with the CORRECT PoP (same sync key, second device)
// is accepted — this is the legitimate cross-device scenario.
// ---------------------------------------------------------------------------

/// Two devices that share the same sync key (same account) must both be able
/// to co-register the shared inbox and each receive an independent auth token.
#[tokio::test]
async fn coregistration_with_correct_pop_is_accepted() {
    let (app, _state) = make_app();

    // First device registers.
    let pop = compute_pop(&SYNC_KEY, DEVICE_ID);
    let first = json!({
        "device_id": DEVICE_ID,
        "device_name": "Device One",
        "public_key_b64": valid_pubkey(),
        "pop_b64": pop.clone(),
    });
    let (status1, body1) = post_register(app.clone(), first).await;
    assert_eq!(
        status1,
        StatusCode::CREATED,
        "first registration must succeed"
    );
    let token1 = body1["auth_token"].as_str().unwrap().to_owned();

    // Second device on the same account co-registers with the same PoP.
    let second = json!({
        "device_id": DEVICE_ID,
        "device_name": "Device Two",
        "public_key_b64": valid_pubkey(),
        "pop_b64": pop,
    });
    let (status2, body2) = post_register(app, second).await;
    assert_eq!(
        status2,
        StatusCode::CREATED,
        "co-registration with correct pop_b64 must succeed with 201; got {status2} body={body2}"
    );
    let token2 = body2["auth_token"].as_str().unwrap().to_owned();

    // Each device gets an INDEPENDENT token.
    assert_ne!(
        token1, token2,
        "co-registered devices must receive independent tokens"
    );
}

// ---------------------------------------------------------------------------
// Test 5: registration with pop_b64 that is valid base64 but the wrong length
// (not 32 bytes decoded) is rejected.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn registration_with_malformed_pop_is_rejected() {
    let (app, _state) = make_app();
    // Valid base64 but decodes to only 16 bytes (not 32).
    let bad_pop = B64.encode([0u8; 16]);
    let body = json!({
        "device_id": DEVICE_ID,
        "device_name": "Test Device",
        "public_key_b64": valid_pubkey(),
        "pop_b64": bad_pop,
    });
    let (status, resp_body) = post_register(app, body).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "pop_b64 decoding to wrong length must be rejected; got {status} body={resp_body}"
    );
}
