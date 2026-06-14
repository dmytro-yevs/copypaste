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

/// Like [`make_app`] but with a larger `max_item_bytes` so the request-body
/// limit (`max_item_bytes + 4096`) does not preempt the per-content-type tier
/// caps. Needed to exercise the 8 MiB text cap: base64 of >8 MiB exceeds the
/// default 10 MiB body limit, which would 413 at the body layer (wrong code)
/// before the item-size quota runs.
fn make_app_with_item_bytes(max_item_bytes: usize) -> (axum::Router, AppState) {
    let config = RelayConfig {
        max_item_bytes,
        ..RelayConfig::default()
    };
    let store = RelayStore::new(config.sync_ttl_secs);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = relay_router(app_state.clone(), config);
    (router, app_state)
}

fn valid_pub_key() -> String {
    B64.encode([0u8; 32])
}
/// Dummy 32-byte proof-of-possession for integration tests. Tests that do not
/// exercise PoP semantics use this sentinel (any 32-byte value is accepted on
/// first registration; co-registration with the same value succeeds).
fn valid_pop() -> String {
    B64.encode([0xDE_u8; 32])
}

const DEVICE_A: &str = "11111111-1111-1111-1111-111111111111";
// Reserved for multi-device tests; not used in every test case.
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
        "pop_b64": valid_pop(),
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
async fn test_co_registration_returns_201_and_new_token() {
    // R1a shared-account co-registration: re-registering an already-registered
    // device_id no longer returns 409 — it returns 201 with a fresh, distinct
    // auth_token, and BOTH tokens authorize pull on the shared inbox.
    let (app, _state) = make_app();
    let (status1, body1, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(status1, StatusCode::CREATED);
    let (status2, body2, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    assert_eq!(
        status2,
        StatusCode::CREATED,
        "co-registration of a known device_id must succeed (201), not 409"
    );
    let token1 = body1["auth_token"].as_str().unwrap().to_string();
    let token2 = body2["auth_token"].as_str().unwrap().to_string();
    assert_ne!(token1, token2, "co-registration must mint a distinct token");
    assert_eq!(body2["device_id"], DEVICE_A);

    // Both tokens authorize a pull against the shared inbox.
    let (s1, _) = get_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token1),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "original token must authorize pull");
    let (s2, _) = get_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token2),
    )
    .await;
    assert_eq!(
        s2,
        StatusCode::OK,
        "co-registered token must authorize pull"
    );
}

#[tokio::test]
async fn test_co_registered_tokens_share_one_inbox() {
    // The core cross-device-delivery property (R1a): an item PUSHED with one
    // co-registered token is READABLE via a different co-registered token from
    // the same shared inbox. This is what lets one device's item reach another.
    let (app, _state) = make_app();
    let (_, body1, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    let (_, body2, app) = register_device(app, DEVICE_A, &valid_pub_key()).await;
    let token1 = body1["auth_token"].as_str().unwrap().to_string();
    let token2 = body2["auth_token"].as_str().unwrap().to_string();

    // Push with token1.
    let (push_status, _) = post_json(
        app.clone(),
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token1),
        json!({ "content_type": "text", "content_b64": "aGVsbG8=", "wall_time": 1000u64 }),
    )
    .await;
    assert_eq!(push_status, StatusCode::CREATED);

    // Read with token2 — the item is visible on the shared inbox.
    let (pull_status, pull_body) =
        get_json(app, &format!("/devices/{DEVICE_A}/items"), Some(&token2)).await;
    assert_eq!(pull_status, StatusCode::OK);
    let items = pull_body.as_array().expect("pull returns a JSON array");
    assert_eq!(items.len(), 1, "token2 must see the item token1 pushed");
    assert_eq!(items[0]["content_b64"], "aGVsbG8=");
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
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
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
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
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
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
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
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };
    let del_status = delete_req(app, &format!("/devices/{DEVICE_A}/items/9999"), &a_token).await;
    assert_eq!(del_status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Item-size quota (plan: 413 ITEM_SIZE_EXCEEDED, checked at the HTTP layer)
// ---------------------------------------------------------------------------

/// A text item whose decoded ciphertext exceeds the Free-tier 8 MiB text limit
/// must be rejected with HTTP 413 and the `ITEM_SIZE_EXCEEDED` error code. We
/// raise `max_item_bytes` so the request-body limit does not preempt the tier
/// check (base64 of >8 MiB would otherwise exceed the default 10 MiB body cap).
#[tokio::test]
async fn test_push_oversized_text_item_is_413_item_size_exceeded() {
    let (app, state) = make_app_with_item_bytes(20 * 1024 * 1024);
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // 8 MiB + 1 byte of decoded payload — over the Free text limit (8 MiB),
    // under the raised 20 MiB body/max_item_bytes limit.
    let oversized = B64.encode(vec![0u8; 8 * 1024 * 1024 + 1]);
    let (status, body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({"content_type": "text", "content_b64": oversized, "wall_time": 1000}),
    )
    .await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["code"], "ITEM_SIZE_EXCEEDED");
}

/// A text item at exactly the 8 MiB Free-tier limit must be accepted.
#[tokio::test]
async fn test_push_text_item_at_limit_is_accepted() {
    let (app, state) = make_app_with_item_bytes(20 * 1024 * 1024);
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Exactly 8 MiB of decoded payload — at the limit, must pass.
    let at_limit = B64.encode(vec![0u8; 8 * 1024 * 1024]);
    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({"content_type": "text", "content_b64": at_limit, "wall_time": 1000}),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
}

// ---------------------------------------------------------------------------
// last_seen wiring — handler-level proof that push/pull/delete advance it
// ---------------------------------------------------------------------------

/// Prove that the `push` route handler calls `update_last_seen` after a
/// successful authenticated request.
///
/// The test rewinds `last_seen` on the device record to a time well past the
/// cleanup threshold, then hits the push route.  After the route returns, it
/// reads `last_seen` directly from the store and asserts it has advanced to
/// (approximately) "now".  A device that has had its `last_seen` refreshed
/// this way must survive `cleanup_inactive_devices` — proving the wiring
/// between the HTTP handler and the store method is intact.
///
/// This guards against `update_last_seen` being inadvertently removed from
/// `routes/items.rs`: if the call is dropped, `last_seen` stays at the rewound
/// value and the device is evicted by cleanup despite being active.
#[tokio::test]
async fn test_push_route_advances_last_seen() {
    use std::time::{Duration, Instant};

    let (app, state) = make_app();

    // Register a device directly in the store (no HTTP round-trip needed for setup).
    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Rewind last_seen to 2 hours ago — well past any realistic cleanup threshold.
    let rewind = Duration::from_secs(2 * 3600);
    {
        let mut s = state.lock().unwrap();
        let record = s.devices.get_mut(DEVICE_A).unwrap();
        record.last_seen = Instant::now() - rewind;
    }

    // Capture a timestamp just before the HTTP request so we can assert that
    // `last_seen` ends up >= this instant after the route runs.
    let before = Instant::now();

    // Hit the push route with a valid token and a minimal text payload.
    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&token),
        json!({
            "content_type": "text",
            "content_b64": sample_content_b64(),
            "wall_time": 1000u64,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "push must succeed");

    // Read last_seen back from the store.
    let last_seen = {
        let s = state.lock().unwrap();
        s.devices.get(DEVICE_A).unwrap().last_seen
    };

    // last_seen must have been advanced past the rewound value.
    assert!(
        last_seen >= before,
        "push route must advance last_seen to >= the instant before the request \
         (got elapsed since before = {:?})",
        before.elapsed()
    );

    // Concretely: the device must now SURVIVE cleanup with a 1-hour threshold
    // because last_seen was just refreshed to "now".
    let threshold_secs = 3600u64; // 1 hour
    let removed = {
        let mut s = state.lock().unwrap();
        s.cleanup_inactive_devices(threshold_secs)
    };
    assert_eq!(
        removed, 0,
        "device whose last_seen was just refreshed by push must survive cleanup"
    );
}

/// Prove that the `pull` route handler calls `update_last_seen`.
///
/// Mirrors `test_push_route_advances_last_seen` for the GET /items route:
/// after rewinding `last_seen` and issuing an authenticated pull, the device
/// must survive `cleanup_inactive_devices`.
#[tokio::test]
async fn test_pull_route_advances_last_seen() {
    use std::time::{Duration, Instant};

    let (app, state) = make_app();

    let token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // Rewind last_seen to 2 hours ago.
    let rewind = Duration::from_secs(2 * 3600);
    {
        let mut s = state.lock().unwrap();
        let record = s.devices.get_mut(DEVICE_A).unwrap();
        record.last_seen = Instant::now() - rewind;
    }

    let before = Instant::now();

    // Hit the pull route.
    let (status, _body) = get_json(
        app,
        &format!("/devices/{DEVICE_A}/items?since=0"),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pull must succeed");

    let last_seen = {
        let s = state.lock().unwrap();
        s.devices.get(DEVICE_A).unwrap().last_seen
    };

    assert!(
        last_seen >= before,
        "pull route must advance last_seen to >= the instant before the request"
    );

    let threshold_secs = 3600u64;
    let removed = {
        let mut s = state.lock().unwrap();
        s.cleanup_inactive_devices(threshold_secs)
    };
    assert_eq!(
        removed, 0,
        "device whose last_seen was just refreshed by pull must survive cleanup"
    );
}

/// An image item up to the 10 MiB Free-tier limit must be accepted — the size
/// check is content-type aware (image cap 10 MiB, text cap 8 MiB).
#[tokio::test]
async fn test_push_large_image_under_image_limit_is_accepted() {
    let (app, state) = make_app();
    let a_token = {
        let mut s = state.lock().unwrap();
        s.register_device(
            DEVICE_A.to_string(),
            "Device A".into(),
            valid_pub_key(),
            valid_pop(),
        )
        .unwrap()
        .0
    };

    // 2 MiB image: well under both the 8 MiB text cap and the 10 MiB image cap.
    let image = B64.encode(vec![0u8; 2 * 1024 * 1024]);
    let (status, _body) = post_json(
        app,
        &format!("/devices/{DEVICE_A}/items"),
        Some(&a_token),
        json!({"content_type": "image", "content_b64": image, "wall_time": 1000}),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
}
