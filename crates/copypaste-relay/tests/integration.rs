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

#[path = "../src/auth.rs"]
mod auth;
#[path = "../src/config.rs"]
mod config;
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

fn relay_router(state: AppState, config: RelayConfig) -> axum::Router {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/health", get(routes_health::handle))
        .route("/devices", post(routes_devices::register))
        .route("/devices/{device_id}", get(routes_devices::get_device))
        .route(
            "/devices/{device_id}/items",
            get(routes_items::pull).post(routes_items::push),
        )
        .route(
            "/devices/{device_id}/items/{item_id}",
            delete(routes_items::delete_item),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
        // Mirror the production router (HIGH #2): inject RelayConfig so
        // `items::push` can read `max_item_bytes` from it.
        .layer(axum::Extension(config))
}

// ---------------------------------------------------------------------------
// Helper utilities
// ---------------------------------------------------------------------------

fn make_app() -> (axum::Router, AppState) {
    let config = RelayConfig::default();
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);
    (router, app_state)
}

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}

const DEVICE_A: &str = "11111111-1111-1111-1111-111111111111";
#[allow(dead_code)]
const DEVICE_B: &str = "22222222-2222-2222-2222-222222222222";

fn sample_content_b64() -> String {
    B64.encode(b"encrypted-clipboard-content")
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
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
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
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn delete_req(app: axum::Router, uri: &str, token: &str) -> StatusCode {
    let req = Request::builder()
        .method(Method::DELETE)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    app.oneshot(req).await.unwrap().status()
}

/// Register a device via HTTP POST /devices. Returns (status, body, app).
async fn register_device(
    app: axum::Router,
    device_id: &str,
    public_key: &str,
) -> (StatusCode, Value, axum::Router) {
    let body = json!({
        "device_id": device_id,
        "device_name": "Test Device",
        "public_key_b64": public_key,
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body, app)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_endpoint() {
    let (app, _state) = make_app();
    let (status, body) = get_json(app, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_register_device_success() {
    let (app, _state) = make_app();
    let (status, body, _) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["device_id"], DEVICE_A);
    assert!(body["auth_token"].as_str().unwrap().len() == 32);
    assert!(body["expires_at"].as_str().is_some());
}

#[tokio::test]
async fn test_register_duplicate_is_409() {
    let (app, _state) = make_app();
    let (_, _, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    let (status, _body, _) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_invalid_uuid_is_400() {
    let (app, _state) = make_app();
    let (status, body, _) = register_device(app, "not-a-uuid", &valid_pub_key()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["code"].as_str().unwrap().contains("BAD_REQUEST"));
}

#[tokio::test]
async fn test_register_invalid_base64_key_is_400() {
    let (app, _state) = make_app();
    let (status, _body, _) = register_device(app, DEVICE_A, "!!!not-base64!!!").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_push_requires_auth() {
    let (app, _state) = make_app();
    let (_, _, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;

    let (status, _) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        None,
        json!({"content_type": "text", "content_b64": sample_content_b64(), "wall_time": 1000}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_pull_requires_auth() {
    let (app, _state) = make_app();
    let (_, _, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    let (status, _) = get_json(app, &format!("/devices/{DEVICE_A}/items"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_push_and_pull_roundtrip() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), "Device A".into(), valid_pub_key())
            .unwrap()
            .0
    };

    // Push an item.
    let (push_status, push_body) = post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000,
        }),
    )
    .await;
    assert_eq!(push_status, StatusCode::CREATED);
    let id = push_body["id"].as_i64().unwrap();
    assert!(id >= 1);

    // Pull it back.
    let (pull_status, pull_body) = get_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
    )
    .await;
    assert_eq!(pull_status, StatusCode::OK);
    let items = pull_body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], id);
    assert_eq!(items[0]["content_type"], "text");
    assert_eq!(items[0]["wall_time"], 1000);

    // Delete it.
    let del_status = delete_req(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items/{id}"),
        &a_token,
    )
    .await;
    assert_eq!(del_status, StatusCode::NO_CONTENT);

    // Pull again — empty.
    let (pull2_status, pull2_body) =
        get_json(app, &format!("/devices/{DEVICE_A}/items"), Some(&a_token)).await;
    assert_eq!(pull2_status, StatusCode::OK);
    assert_eq!(pull2_body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_pull_since_wall_time() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), "Device A".into(), valid_pub_key())
            .unwrap()
            .0
    };

    for wt in [1000u64, 2000, 3000] {
        post_json(
            app.clone(),
            &format!("/devices/{DEVICE_A}/items"),
            Some(&a_token),
            json!({"content_type": "text", "content_b64": sample_content_b64(), "wall_time": wt}),
        )
        .await;
    }

    // since=1000 should return wall_time 2000 and 3000.
    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items?since=1000"),
        Some(&a_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["wall_time"], 2000);
    assert_eq!(items[1]["wall_time"], 3000);
}

#[tokio::test]
async fn test_push_invalid_content_type_is_400() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), "Device A".into(), valid_pub_key())
            .unwrap()
            .0
    };
    let (status, _) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({"content_type": "video", "content_b64": sample_content_b64(), "wall_time": 1000}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_nonexistent_item_is_404() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), "Device A".into(), valid_pub_key())
            .unwrap()
            .0
    };
    let del_status = delete_req(app, &format!("/devices/{DEVICE_A}/items/9999"), &a_token).await;
    assert_eq!(del_status, StatusCode::NOT_FOUND);
}
