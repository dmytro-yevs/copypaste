//! Integration tests for copypaste-relay using axum's oneshot testing pattern.
//!
//! Uses `tower::ServiceExt::oneshot` + `http-body-util` instead of `axum-test`
//! to avoid the `time-core 0.1.8` edition2024 incompatibility on Rust 1.75.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

// Pull in relay modules via path — the relay is a binary crate, so tests
// must use `#[path]` to access internal modules.
#[path = "../src/config.rs"]
mod config;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/state.rs"]
mod state;
#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/routes/health.rs"]
mod routes_health;
#[path = "../src/routes/devices.rs"]
mod routes_devices;
#[path = "../src/routes/items.rs"]
mod routes_items;

use config::RelayConfig;
use state::{AppState, RelayItem, RelayStore};

fn relay_router(state: AppState, config: RelayConfig) -> axum::Router {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/health", get(routes_health::handle))
        .route("/devices", post(routes_devices::register))
        .route(
            "/devices/:device_id/items",
            get(routes_items::poll).post(routes_items::upload),
        )
        .route(
            "/devices/:device_id/items/:item_id",
            delete(routes_items::delete_item),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
}

// ---------------------------------------------------------------------------
// Helper utilities
// ---------------------------------------------------------------------------

fn make_app() -> (axum::Router, AppState) {
    let config = RelayConfig { sync_ttl_secs: 3600, ..RelayConfig::default() };
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);
    (router, app_state)
}

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}

fn valid_nonce() -> String {
    B64.encode([0u8; 24])
}

const DEVICE_A: &str = "11111111-1111-1111-1111-111111111111";
const DEVICE_B: &str = "22222222-2222-2222-2222-222222222222";

/// Send a POST /devices request and return the parsed JSON body + status.
async fn register_device(
    app: axum::Router,
    device_id: &str,
    public_key: &str,
) -> (StatusCode, Value, axum::Router) {
    let body = json!({ "device_id": device_id, "public_key": public_key });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json, app)
}

async fn get_json(app: axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(Method::GET).uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = builder.body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn post_json(
    app: axum::Router,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = builder
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn delete_req(app: axum::Router, uri: &str, token: &str) -> StatusCode {
    let req = Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    resp.status()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_empty() {
    let (app, _state) = make_app();
    let (status, body) = get_json(app, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["devices"], 0);
    assert_eq!(body["total_items"], 0);
}

#[tokio::test]
async fn test_register_device() {
    let (app, _state) = make_app();
    let (status, body, _app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["device_id"], DEVICE_A);
    let token = body["bearer_token"].as_str().unwrap();
    assert_eq!(token.len(), 32);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_register_duplicate_is_409() {
    let (app, state) = make_app();
    // Register via state directly to share the same Arc
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let (status, _body, _app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_invalid_public_key_is_400() {
    let (app, _state) = make_app();
    // 31 bytes — not 32
    let short_key = B64.encode([0u8; 31]);
    let (status, _body, _app) = register_device(app, DEVICE_A, &short_key).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_poll_requires_auth() {
    let (app, state) = make_app();
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let (status, _body) =
        get_json(app, &format!("/devices/{DEVICE_A}/items"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_upload_and_poll_roundtrip() {
    let (app, state) = make_app();

    let (a_token, b_token) = {
        let mut s = state.lock().unwrap();
        let a = s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
        let b = s
            .register_device(DEVICE_B.to_string(), B64.encode([1u8; 32]))
            .unwrap();
        (a, b)
    };

    const ITEM_ID: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

    // A uploads
    let (upload_status, upload_body) = post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "item_id": ITEM_ID,
            "ciphertext_b64": B64.encode(b"encrypted"),
            "nonce_b64": valid_nonce(),
            "sender_device_id": DEVICE_A,
            "lamport_ts": 1,
            "content_type": "text"
        }),
    )
    .await;
    assert_eq!(upload_status, StatusCode::CREATED);
    assert_eq!(upload_body["fanned_out_to"], 1);

    // B polls and sees the item
    let (poll_status, poll_body) =
        get_json(app.clone(), &format!("/devices/{DEVICE_B}/items"), Some(&b_token)).await;
    assert_eq!(poll_status, StatusCode::OK);
    assert_eq!(poll_body["items"].as_array().unwrap().len(), 1);

    // B deletes the item
    let del_status = delete_req(
        app.clone(),
        &format!("/devices/{DEVICE_B}/items/{ITEM_ID}"),
        &b_token,
    )
    .await;
    assert_eq!(del_status, StatusCode::NO_CONTENT);

    // B polls again — empty
    let (poll2_status, poll2_body) =
        get_json(app.clone(), &format!("/devices/{DEVICE_B}/items"), Some(&b_token)).await;
    assert_eq!(poll2_status, StatusCode::OK);
    assert_eq!(poll2_body["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_upload_invalid_nonce_is_400() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let bad_nonce = B64.encode([0u8; 23]); // 23 bytes, not 24
    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "item_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ciphertext_b64": B64.encode(b"data"),
            "nonce_b64": bad_nonce,
            "sender_device_id": DEVICE_A,
            "lamport_ts": 1,
            "content_type": "text"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_poll_since_lamport() {
    let (app, state) = make_app();

    let (a_token, b_token) = {
        let mut s = state.lock().unwrap();
        let a = s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
        let b = s
            .register_device(DEVICE_B.to_string(), B64.encode([1u8; 32]))
            .unwrap();
        (a, b)
    };

    // Upload 3 items with lamport_ts 1, 2, 3
    for ts in [1u64, 2, 3] {
        post_json(
            app.clone(),
            &format!("/devices/{DEVICE_A}/items"),
            Some(&a_token),
            json!({
                "item_id": format!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaa{:04}", ts),
                "ciphertext_b64": B64.encode(b"x"),
                "nonce_b64": valid_nonce(),
                "sender_device_id": DEVICE_A,
                "lamport_ts": ts,
                "content_type": "text"
            }),
        )
        .await;
    }

    // Poll since_lamport=1 should return ts=2 and ts=3
    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_B}/items?since_lamport=1"),
        Some(&b_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["lamport_ts"], 2);
    assert_eq!(items[1]["lamport_ts"], 3);
}

#[tokio::test]
async fn test_ttl_expiry() {
    // Use sync_ttl_secs=0 so items expire immediately
    let config = RelayConfig { sync_ttl_secs: 0, ..RelayConfig::default() };
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let app = relay_router(app_state.clone(), config.clone());

    let (a_token, b_token) = {
        let mut s = app_state.lock().unwrap();
        let a = s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
        let b = s
            .register_device(DEVICE_B.to_string(), B64.encode([1u8; 32]))
            .unwrap();
        (a, b)
    };

    // Insert item directly with ttl=0 config so it expires on first poll
    {
        let mut s = app_state.lock().unwrap();
        let item = RelayItem {
            item_id: "ttl-test-item".to_string(),
            ciphertext_b64: B64.encode(b"data"),
            nonce_b64: B64.encode([0u8; 24]),
            sender_device_id: DEVICE_A.to_string(),
            lamport_ts: 1,
            content_type: "text".to_string(),
            uploaded_at: std::time::Instant::now(),
        };
        s.upload_item(item, &config);
    }

    // Upload via HTTP also (belt-and-suspenders)
    post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "item_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ciphertext_b64": B64.encode(b"x"),
            "nonce_b64": valid_nonce(),
            "sender_device_id": DEVICE_A,
            "lamport_ts": 2,
            "content_type": "text"
        }),
    )
    .await;

    // Poll immediately — with ttl=0, elapsed >= 0 is always true, items pruned
    let (status, body) =
        get_json(app, &format!("/devices/{DEVICE_B}/items"), Some(&b_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_upload_fanout_excludes_sender() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    // Upload (only device A registered — no other device to fan out to)
    post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "item_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ciphertext_b64": B64.encode(b"data"),
            "nonce_b64": valid_nonce(),
            "sender_device_id": DEVICE_A,
            "lamport_ts": 1,
            "content_type": "text"
        }),
    )
    .await;

    // A polls its OWN inbox — must be empty (sender excluded from fan-out)
    let (status, body) =
        get_json(app, &format!("/devices/{DEVICE_A}/items"), Some(&a_token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}
