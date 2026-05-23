#![allow(dead_code)]
//! Integration tests for the Prometheus `/metrics` endpoint.
//!
//! Mirrors the path-rebind pattern from `tests/integration.rs` so the
//! test target can build without depending on the binary `crate` root.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

#[path = "../src/config.rs"]
mod config;
#[path = "../src/error.rs"]
mod error;
#[path = "../src/api/metrics.rs"]
mod metrics_handler;
#[path = "../src/models.rs"]
mod models;
#[path = "../src/quota.rs"]
mod quota;
#[path = "../src/state.rs"]
mod state;

use state::{AppState, RelayStore};

// ---------------------------------------------------------------------------
// Test harness — minimal router with only /metrics wired
// ---------------------------------------------------------------------------

fn metrics_only_router(state: AppState) -> axum::Router {
    use axum::routing::get;
    axum::Router::new()
        .route("/metrics", get(metrics_handler::handle))
        .with_state(state)
}

fn make_app() -> (axum::Router, AppState) {
    let store = RelayStore::new(3600);
    let app_state: AppState = Arc::new(Mutex::new(store));
    let router = metrics_only_router(app_state.clone());
    (router, app_state)
}

const DEVICE_A: &str = "11111111-1111-1111-1111-111111111111";
const DEVICE_B: &str = "22222222-2222-2222-2222-222222222222";

fn pub_key(seed: u8) -> String {
    B64.encode([seed; 32])
}

async fn fetch_metrics(router: &axum::Router) -> (StatusCode, String, String) {
    let req = Request::builder()
        .method("GET")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    (status, content_type, body)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    let (router, _state) = make_app();
    let (status, content_type, body) = fetch_metrics(&router).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("text/plain"),
        "expected Prometheus text/plain content-type, got {content_type:?}"
    );
    assert!(content_type.contains("version=0.0.4"));

    // All three metric families must appear with HELP + TYPE + value lines.
    for metric in [
        "copypaste_relay_items_total",
        "copypaste_relay_evictions_total",
        "copypaste_relay_active_devices",
    ] {
        assert!(
            body.contains(&format!("# HELP {metric} ")),
            "missing HELP for {metric} in body:\n{body}"
        );
        assert!(
            body.contains(&format!("# TYPE {metric} ")),
            "missing TYPE for {metric}"
        );
        assert!(
            body.contains(&format!("{metric} 0")),
            "expected initial value 0 for {metric}"
        );
    }

    assert!(body.contains("# TYPE copypaste_relay_items_total counter"));
    assert!(body.contains("# TYPE copypaste_relay_evictions_total counter"));
    assert!(body.contains("# TYPE copypaste_relay_active_devices gauge"));
}

#[tokio::test]
async fn counters_increment_on_insert_and_evict() {
    let (router, state) = make_app();

    // Bootstrap: register two devices and push items into each inbox.
    {
        let mut store = state.lock().unwrap();
        store
            .register_device(DEVICE_A.into(), "A".into(), pub_key(1))
            .unwrap();
        store
            .register_device(DEVICE_B.into(), "B".into(), pub_key(2))
            .unwrap();

        // Three pushes into A, two into B — items_total should be 5.
        for wt in [100u64, 200, 300] {
            store
                .push_item(
                    DEVICE_A,
                    "text".into(),
                    B64.encode(b"hi"),
                    wt,
                    10 * 1024 * 1024,
                )
                .unwrap();
        }
        for wt in [10u64, 20] {
            store
                .push_item(
                    DEVICE_B,
                    "text".into(),
                    B64.encode(b"hi"),
                    wt,
                    10 * 1024 * 1024,
                )
                .unwrap();
        }
    }

    // After 5 pushes: items_total=5, evictions_total=0, active_devices=2.
    let (_, _, body) = fetch_metrics(&router).await;
    assert!(
        body.contains("copypaste_relay_items_total 5"),
        "expected items_total=5 after 5 pushes, body:\n{body}"
    );
    assert!(
        body.contains("copypaste_relay_evictions_total 0"),
        "expected evictions_total=0 before any prune"
    );
    assert!(
        body.contains("copypaste_relay_active_devices 2"),
        "expected active_devices=2 (both inboxes non-empty)"
    );

    // Force-evict everything via a far-future "now" with ttl=1 — cutoff
    // (now - 1) still lies far beyond any inserted_at_unix recorded
    // moments ago, so every item is older than the cutoff and pruned.
    let evicted = {
        let mut store = state.lock().unwrap();
        store.prune_expired(u64::MAX / 2, 1)
    };
    assert_eq!(evicted, 5, "all 5 items must be evicted");

    // After eviction: items_total still 5 (counter), evictions_total=5,
    // active_devices=0 (both inboxes drained).
    let (_, _, body) = fetch_metrics(&router).await;
    assert!(
        body.contains("copypaste_relay_items_total 5"),
        "items_total must NOT decrement on eviction (counter), body:\n{body}"
    );
    assert!(
        body.contains("copypaste_relay_evictions_total 5"),
        "evictions_total must reflect 5 dropped items"
    );
    assert!(
        body.contains("copypaste_relay_active_devices 0"),
        "active_devices must drop to 0 once both inboxes are empty"
    );
}
