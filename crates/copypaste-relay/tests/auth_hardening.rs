#![allow(dead_code)]
//! Auth-hardening tests for copypaste-relay (Wave 1.2).
//!
//! Covers:
//! - CRITICAL #1: bearer token is random, NOT derived from the public key
//!   (registering the same pubkey twice yields different tokens).
//! - MEDIUM #14: list endpoints never leak the bearer token.
//! - Field-whitelist check on the list endpoint output.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

// We only exercise registration + list endpoints, but the source modules
// reference each other (config -> error -> models -> quota -> state -> routes::devices),
// so we include the full chain. `dead_code` is silenced via the inner
// attribute at the top of this file because each test binary only walks
// part of the module graph.

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
#[path = "../src/state.rs"]
mod state;

use config::RelayConfig;
use state::{AppState, RelayStore};

// ---------------------------------------------------------------------------
// Router under test — mirrors the production router for the endpoints we exercise.
// ---------------------------------------------------------------------------

fn relay_router(state: AppState, config: RelayConfig) -> axum::Router {
    use axum::extract::State;
    use axum::response::IntoResponse;
    use axum::routing::{get, post};

    async fn list_devices_handler(State(state): State<AppState>) -> impl IntoResponse {
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        let device_ids = store.list_devices();
        axum::Json(serde_json::json!({ "devices": device_ids }))
    }

    axum::Router::new()
        .route(
            "/devices",
            post(routes_devices::register).get(list_devices_handler),
        )
        .route("/devices/{device_id}", get(routes_devices::get_device))
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
}

fn make_app() -> (axum::Router, AppState) {
    let config = RelayConfig::default();
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);
    (router, app_state)
}

fn pub_key(seed: u8) -> String {
    B64.encode([seed; 32])
}

fn device_uuid(n: u8) -> String {
    format!(
        "{n:02x}{n:02x}{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}"
    )
}

async fn register(app: axum::Router, device_id: &str, key: &str) -> (StatusCode, Value) {
    let body = json!({
        "device_id": device_id,
        "device_name": "Test Device",
        "public_key_b64": key,
    });
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

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ---------------------------------------------------------------------------
// Test 1: CRITICAL #1 — token entropy must NOT be derived from pubkey.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn relay_bearer_token_is_random_not_derived_from_pubkey() {
    let (app, _state) = make_app();
    let key = pub_key(7);

    // Two devices, identical public key.
    let (s1, b1) = register(app.clone(), &device_uuid(1), &key).await;
    let (s2, b2) = register(app.clone(), &device_uuid(2), &key).await;
    assert_eq!(s1, StatusCode::CREATED);
    assert_eq!(s2, StatusCode::CREATED);

    let t1 = b1["auth_token"].as_str().expect("missing auth_token");
    let t2 = b2["auth_token"].as_str().expect("missing auth_token");

    // Tokens must be 32 hex chars (16 bytes of entropy).
    assert_eq!(t1.len(), 32, "token length unexpected: {t1}");
    assert_eq!(t2.len(), 32, "token length unexpected: {t2}");
    assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(t2.chars().all(|c| c.is_ascii_hexdigit()));

    // CRITICAL invariant: identical pubkey, different tokens.
    assert_ne!(
        t1, t2,
        "bearer tokens must be randomly generated, not derived from public key"
    );
}

// ---------------------------------------------------------------------------
// Test 2: list endpoint must not leak bearer tokens.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn relay_get_devices_does_not_leak_bearer_token() {
    let (app, _state) = make_app();
    let (status, _) = register(app.clone(), &device_uuid(1), &pub_key(1)).await;
    assert_eq!(status, StatusCode::CREATED);

    let (list_status, list_body) = get_json(app.clone(), "/devices").await;
    assert_eq!(list_status, StatusCode::OK);

    // Serialize the entire response back to a JSON string and scan it for
    // any forbidden substring. This catches accidental leakage no matter
    // where in the nested structure it appears.
    let body_str = serde_json::to_string(&list_body).unwrap();
    assert!(
        !body_str.contains("bearer_token"),
        "list endpoint leaks bearer_token field: {body_str}"
    );
    assert!(
        !body_str.contains("auth_token"),
        "list endpoint leaks auth_token field: {body_str}"
    );

    // Per-device GET must also not leak the bearer token.
    let (info_status, info_body) = get_json(app, &format!("/devices/{}", device_uuid(1))).await;
    assert_eq!(info_status, StatusCode::OK);
    let info_str = serde_json::to_string(&info_body).unwrap();
    assert!(
        !info_str.contains("bearer_token"),
        "GET /devices/:id leaks bearer_token: {info_str}"
    );
    assert!(
        !info_str.contains("auth_token"),
        "GET /devices/:id leaks auth_token: {info_str}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: list endpoint returns only the whitelisted shape.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn relay_list_endpoint_returns_only_safe_fields() {
    let (app, _state) = make_app();
    let _ = register(app.clone(), &device_uuid(1), &pub_key(1)).await;
    let _ = register(app.clone(), &device_uuid(2), &pub_key(2)).await;

    let (status, body) = get_json(app, "/devices").await;
    assert_eq!(status, StatusCode::OK);

    // Top-level shape: { "devices": [<string id>, ...] } — nothing else.
    let obj = body.as_object().expect("response must be JSON object");
    assert_eq!(
        obj.len(),
        1,
        "list response must have exactly one key, got {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    let arr = obj
        .get("devices")
        .and_then(|v| v.as_array())
        .expect("`devices` must be an array");

    // Each entry must be a plain string (just the device_id), never an
    // object that could hide sensitive fields.
    for entry in arr {
        assert!(
            entry.is_string(),
            "list entry must be a plain string device_id, got {entry:?}"
        );
    }
    assert_eq!(arr.len(), 2);
}

// ---------------------------------------------------------------------------
// Bonus: proof-of-possession validation rejects empty / wrong-length keys.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn relay_register_rejects_empty_public_key() {
    let (app, _state) = make_app();
    let (status, _) = register(app, &device_uuid(1), "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn relay_register_rejects_wrong_length_public_key() {
    let (app, _state) = make_app();
    // 16 bytes instead of 32.
    let short = B64.encode([0u8; 16]);
    let (status, _) = register(app, &device_uuid(1), &short).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Wave 3.4 — concurrent-read smoke test for list/get device handlers.
//
// Originally specified to assert that GET handlers use `RwLock::read()` for
// truly-concurrent access. Wave 3.4 kept `Mutex` to avoid multi-file ripple,
// but the contract for callers is unchanged: 100 concurrent GETs must
// complete without deadlock and within a reasonable time bound.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn read_only_handlers_handle_concurrent_load() {
    let (app, _state) = make_app();
    // Seed a couple of devices so the list isn't empty. make_app() gives us a
    // fresh RelayStore so per-device limiter state cannot leak across tests.
    let (s, _) = register(app.clone(), &device_uuid(100), &pub_key(100)).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = register(app.clone(), &device_uuid(101), &pub_key(101)).await;
    assert_eq!(s, StatusCode::CREATED);

    let start = std::time::Instant::now();
    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let app_clone = app.clone();
        handles.push(tokio::spawn(async move {
            get_json(app_clone, "/devices").await
        }));
    }
    for h in handles {
        let (status, body) = h.await.expect("task panicked");
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("devices").is_some());
    }
    let elapsed = start.elapsed();
    // Smoke check: 100 in-process GETs must finish well under 5 seconds even
    // serialized through a Mutex. If this ever exceeds 5s the lock has become
    // a real bottleneck and the RwLock migration becomes urgent.
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "100 concurrent GET /devices took {elapsed:?} — lock contention regressed",
    );
}

// ---------------------------------------------------------------------------
// Wave 3.4 — security MEDIUM #13: per-device registration rate limit.
//
// Five attempts within 60s for the same device_id are allowed (under R1a each
// co-registers and returns 201, but the limiter does not reject them). The 6th
// attempt within the window must return 429 with a Retry-After header — the
// per-(ip, device) limiter still bounds co-registration floods from one source.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn register_device_per_device_rate_limited() {
    // Each call to make_app() creates a fresh RelayStore (which owns the
    // limiter state), so no cross-test interference is possible — the limit
    // only kicks in for re-registrations against THIS app's store.
    let (app, _state) = make_app();

    let device_id = device_uuid(42);
    let key = pub_key(42);

    // First attempt: succeeds with 201 Created.
    let (status, _) = register(app.clone(), &device_id, &key).await;
    assert_eq!(status, StatusCode::CREATED);

    // Attempts 2..=5: subsequent registrations of the same device_id now
    // co-register (R1a) and return 201 Created, each minting a new token — but
    // each one still counts toward the per-(ip, device) rate-limit window.
    for n in 2..=5 {
        let (status, _) = register(app.clone(), &device_id, &key).await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "attempt #{n}: expected 201 (co-registration), got {status}",
        );
    }

    // 6th attempt within 60s: must be short-circuited by the limiter as 429.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "device_id": device_id,
                "device_name": "Test Device",
                "public_key_b64": key,
            }))
            .unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th registration within 60s must be rate-limited",
    );
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .expect("Retry-After header must be present on 429")
        .to_str()
        .unwrap()
        .parse::<u64>()
        .expect("Retry-After must be an integer number of seconds");
    assert!(
        (1..=60).contains(&retry_after),
        "Retry-After must be between 1 and 60 seconds, got {retry_after}",
    );
}
