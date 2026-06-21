//! CopyPaste-tspz: relay-side watermark header tests.
//!
//! The `pull` handler emits a `Relay-Watermark: <wall_time>,<id>` response
//! header so clients interrupted mid-drain (e.g. by a 401 token expiry) can
//! persist the cursor and resume without discarding already-ingested progress.
//!
//! Tests:
//!  1. Header present and `0,0` on an empty inbox pull.
//!  2. Header contains the last item's `(wall_time, id)` after a non-empty pull.
//!  3. Header advances with each successive page so burst-drain resume is exact.
//!  4. Header reflects `since`/`since_id` params when the inbox is empty.

#![allow(dead_code, unused_imports, unused_variables)]

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
#[path = "../src/routes/items.rs"]
mod routes_items;
#[path = "../src/state.rs"]
mod state;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use config::RelayConfig;
use routes_items::RELAY_WATERMARK_HEADER;
use state::{AppState, RelayStore};

// ── test helpers ─────────────────────────────────────────────────────────────

fn make_state() -> AppState {
    Arc::new(Mutex::new(RelayStore::new(3600)))
}

/// Build a minimal router for integration tests (no GovernorLayer so there
/// is no rate-limiting noise — all requests reach the handler directly).
fn test_router(state: AppState) -> axum::Router {
    use axum::routing::{get, post};
    let config = RelayConfig::default();
    axum::Router::new()
        .route(
            "/devices/{device_id}/items",
            get(routes_items::pull).post(routes_items::push),
        )
        .route("/devices", post(register_handler))
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
        .layer(axum::Extension(config))
}

/// Thin register handler that bypasses the full devices.rs so this test
/// file does not need to `#[path]`-include routes/devices.rs and all of its
/// sub-deps (which would create duplicate symbol issues with the state module
/// already included via routes_items).
async fn register_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::Json(body): axum::Json<Value>,
) -> impl axum::response::IntoResponse {
    let device_id = body["device_id"].as_str().unwrap_or("").to_string();
    let device_name = body["device_name"].as_str().unwrap_or("Test").to_string();
    let pub_key = body["public_key_b64"].as_str().unwrap_or("").to_string();
    let pop = body["pop_b64"].as_str().unwrap_or("").to_string();

    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
    let result = store.register_device(device_id, device_name, pub_key, pop);
    match result {
        Ok((token, _expires)) => (
            StatusCode::CREATED,
            axum::Json(json!({ "auth_token": token })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::Json(json!({ "error": format!("{e}") })),
        ),
    }
}

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}
fn valid_pop() -> String {
    B64.encode([0xDE_u8; 32])
}

/// Register a device and return its auth token.
async fn register(app: axum::Router, device_id: &str) -> String {
    let body = json!({
        "device_id": device_id,
        "device_name": "Test",
        "public_key_b64": valid_pub_key(),
        "pop_b64": valid_pop(),
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/devices")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "register must return 201"
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&bytes).unwrap();
    val["auth_token"].as_str().unwrap().to_string()
}

/// Push a single item and return its id.
async fn push_item(app: axum::Router, device_id: &str, token: &str, wall_time: u64) -> i64 {
    let body = json!({
        "content_type": "text",
        "content_b64": B64.encode(b"hello"),
        "wall_time": wall_time,
    });
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/devices/{device_id}/items"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "push must return 201");
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&bytes).unwrap();
    val["id"].as_i64().unwrap()
}

/// Pull items and return (status, items_json, watermark_header_value).
async fn pull_items(
    app: axum::Router,
    device_id: &str,
    token: &str,
    since: u64,
    since_id: Option<i64>,
) -> (StatusCode, Value, Option<String>) {
    let uri = match since_id {
        Some(sid) => format!("/devices/{device_id}/items?since={since}&since_id={sid}"),
        None => format!("/devices/{device_id}/items?since={since}"),
    };
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let watermark = resp
        .headers()
        .get(RELAY_WATERMARK_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body, watermark)
}

const DEVICE_A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

// ── tests ─────────────────────────────────────────────────────────────────────

/// tspz: empty inbox pull must include `Relay-Watermark: 0,0` (since=0, no
/// since_id means 0).
#[tokio::test]
async fn tspz_watermark_header_present_on_empty_pull() {
    let state = make_state();
    let app = test_router(state);

    let token = register(app.clone(), DEVICE_A).await;
    let (_status, _body, watermark) = pull_items(app, DEVICE_A, &token, 0, None).await;

    assert!(
        watermark.is_some(),
        "CopyPaste-tspz: Relay-Watermark header must be present on every pull response"
    );
    assert_eq!(
        watermark.as_deref(),
        Some("0,0"),
        "CopyPaste-tspz: empty inbox pull from (since=0, since_id=None) must return watermark 0,0"
    );
}

/// tspz: a non-empty pull must include the last item's (wall_time, id) in the
/// `Relay-Watermark` header.
#[tokio::test]
async fn tspz_watermark_header_reflects_last_item_after_nonempty_pull() {
    let state = make_state();
    let app = test_router(state);

    let token = register(app.clone(), DEVICE_A).await;
    // Push two items with distinct wall_times.
    let id1 = push_item(app.clone(), DEVICE_A, &token, 1000).await;
    let id2 = push_item(app.clone(), DEVICE_A, &token, 2000).await;

    let (_status, body, watermark) = pull_items(app, DEVICE_A, &token, 0, None).await;

    assert_eq!(
        body.as_array().map(|a| a.len()),
        Some(2),
        "must return 2 items"
    );

    let expected = format!("2000,{id2}");
    assert_eq!(
        watermark.as_deref(),
        Some(expected.as_str()),
        "CopyPaste-tspz: watermark must be last item's (wall_time={},id={})",
        2000,
        id2
    );
    let _ = id1;
}

/// tspz: when the inbox is empty but `since` / `since_id` are provided (the
/// client is resuming from a saved watermark), the header must echo those
/// values back so they remain valid as a resume cursor.
#[tokio::test]
async fn tspz_watermark_echoes_since_params_on_empty_page() {
    let state = make_state();
    let app = test_router(state);

    let token = register(app.clone(), DEVICE_A).await;
    // Pull with an explicit since/since_id but no items in the inbox past it.
    let (_status, body, watermark) = pull_items(app, DEVICE_A, &token, 5000, Some(42)).await;

    assert_eq!(
        body.as_array().map(|a| a.len()),
        Some(0),
        "inbox is empty past since=5000"
    );
    assert_eq!(
        watermark.as_deref(),
        Some("5000,42"),
        "CopyPaste-tspz: empty pull must echo the incoming cursor back as watermark"
    );
}

/// tspz: burst-drain scenario — the watermark advances strictly with each page
/// so a client interrupted mid-drain can resume from the last received header.
#[tokio::test]
async fn tspz_watermark_advances_with_each_burst_drain_page() {
    let state = make_state();
    let app = test_router(state);

    let token = register(app.clone(), DEVICE_A).await;
    // Push 3 items; wall_time increases.
    let id1 = push_item(app.clone(), DEVICE_A, &token, 100).await;
    let id2 = push_item(app.clone(), DEVICE_A, &token, 200).await;
    let id3 = push_item(app.clone(), DEVICE_A, &token, 300).await;

    // Page 1: pull all 3 items (limit default).
    let (_s, body1, wm1) = pull_items(app.clone(), DEVICE_A, &token, 0, None).await;
    assert_eq!(body1.as_array().map(|a| a.len()), Some(3));
    // Watermark must be the last item.
    let expected_wm1 = format!("300,{id3}");
    assert_eq!(
        wm1.as_deref(),
        Some(expected_wm1.as_str()),
        "tspz: page-1 watermark must be last pushed item"
    );

    // Parse the watermark and use it as the cursor for the next pull.
    let wm1_str = wm1.unwrap();
    let mut parts = wm1_str.split(',');
    let wm_wall: u64 = parts.next().unwrap().parse().unwrap();
    let wm_id: i64 = parts.next().unwrap().parse().unwrap();

    // Page 2: nothing past the cursor — watermark echoes cursor back.
    let (_s, body2, wm2) = pull_items(app, DEVICE_A, &token, wm_wall, Some(wm_id)).await;
    assert_eq!(body2.as_array().map(|a| a.len()), Some(0));
    let expected_wm2 = format!("{wm_wall},{wm_id}");
    assert_eq!(
        wm2.as_deref(),
        Some(expected_wm2.as_str()),
        "tspz: empty page-2 watermark must echo the cursor"
    );
    let _ = (id1, id2);
}
