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
use state::{AppState, RelayStore};

fn relay_router(state: AppState, config: RelayConfig) -> axum::Router {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/health", get(routes_health::handle))
        .route("/devices", post(routes_devices::register))
        .route(
            "/devices/:device_id/items",
            get(routes_items::pull).post(routes_items::push),
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
const DEVICE_B: &str = "22222222-2222-2222-2222-222222222222";

/// Encrypted payload — small enough to fit within quota.
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
// Health
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

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_register_device() {
    let (app, _state) = make_app();
    let body = json!({ "device_id": DEVICE_A, "public_key": valid_pub_key() });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["device_id"], DEVICE_A);
    let token = json["bearer_token"].as_str().unwrap();
    assert_eq!(token.len(), 32);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_register_duplicate_is_409() {
    let (app, state) = make_app();
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let body = json!({ "device_id": DEVICE_A, "public_key": valid_pub_key() });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_invalid_public_key_is_400() {
    let (app, _state) = make_app();
    // 31 bytes — not 32
    let short_key = B64.encode([0u8; 31]);
    let body = json!({ "device_id": DEVICE_A, "public_key": short_key });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pull_requires_auth() {
    let (app, state) = make_app();
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let (status, _body) = get_json(app, &format!("/devices/{DEVICE_A}/items"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_push_requires_auth() {
    let (app, state) = make_app();
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        None,
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_wrong_token_is_401() {
    let (app, state) = make_app();
    {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
    }
    let (status, _body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some("wrongtoken000000000000000000000"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Push → Pull round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_push_returns_id() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let (status, body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].as_i64().is_some(), "response must contain integer id");
}

#[tokio::test]
async fn test_push_and_pull_roundtrip() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    // Push one item.
    let (push_status, push_body) = post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 5000u64
        }),
    )
    .await;
    assert_eq!(push_status, StatusCode::CREATED);
    let pushed_id = push_body["id"].as_i64().unwrap();

    // Pull all items (since=0).
    let (pull_status, pull_body) = get_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
    )
    .await;
    assert_eq!(pull_status, StatusCode::OK);
    let items = pull_body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], pushed_id);
    assert_eq!(items[0]["content_type"], "text");
    assert_eq!(items[0]["content_b64"], sample_content_b64());
    assert_eq!(items[0]["wall_time"], 5000u64);
}

#[tokio::test]
async fn test_pull_since_filters_correctly() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    // Push three items with wall_time 1000, 2000, 3000.
    for wt in [1000u64, 2000, 3000] {
        post_json(
            app.clone(),
            &format!("/devices/{DEVICE_A}/items"),
            Some(&token),
            json!({
                "content_type": "text",
                "content_b64": sample_content_b64(),
                "wall_time": wt
            }),
        )
        .await;
    }

    // Pull since=1000 → should return items with wall_time 2000 and 3000.
    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items?since=1000"),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["wall_time"], 2000u64);
    assert_eq!(items[1]["wall_time"], 3000u64);
}

#[tokio::test]
async fn test_pull_returns_empty_when_no_items() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 0);
}

#[tokio::test]
async fn test_pull_sorted_ascending_by_wall_time() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    // Push in non-sorted order.
    for wt in [3000u64, 1000, 2000] {
        post_json(
            app.clone(),
            &format!("/devices/{DEVICE_A}/items"),
            Some(&token),
            json!({
                "content_type": "text",
                "content_b64": sample_content_b64(),
                "wall_time": wt
            }),
        )
        .await;
    }

    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items?since=0"),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    let times: Vec<u64> = items
        .iter()
        .map(|i| i["wall_time"].as_u64().unwrap())
        .collect();
    assert_eq!(times, vec![1000u64, 2000, 3000]);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_push_invalid_content_type_is_400() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "video",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_push_invalid_base64_is_400() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "text",
            "content_b64": "!!!not-base64!!!",
            "wall_time": 1000u64
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_push_device_isolation() {
    // Items pushed to device A are not visible to device B.
    let (app, state) = make_app();
    let (a_token, b_token) = {
        let mut s = state.lock().unwrap();
        let a = s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap();
        let b = s
            .register_device(DEVICE_B.to_string(), B64.encode([1u8; 32]))
            .unwrap();
        (a, b)
    };

    post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64
        }),
    )
    .await;

    // Device B's inbox should be empty.
    let (status, body) = get_json(
        app,
        &format!("/devices/{DEVICE_B}/items"),
        Some(&b_token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 0, "device B must not see device A's items");
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_item_removes_it() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    // Push an item.
    let (_, push_body) = post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64
        }),
    )
    .await;
    let item_id = push_body["id"].as_i64().unwrap();

    // Delete it.
    let del_status = delete_req(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items/{item_id}"),
        &token,
    )
    .await;
    assert_eq!(del_status, StatusCode::NO_CONTENT);

    // Pull — must be empty now.
    let (_, pull_body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
    )
    .await;
    assert_eq!(pull_body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_delete_nonexistent_item_is_404() {
    let (app, state) = make_app();
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(DEVICE_A.to_string(), valid_pub_key()).unwrap()
    };

    let del_status = delete_req(
        app,
        &format!("/devices/{DEVICE_A}/items/9999"),
        &token,
    )
    .await;
    assert_eq!(del_status, StatusCode::NOT_FOUND);
}
