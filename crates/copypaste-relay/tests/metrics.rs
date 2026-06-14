#![allow(dead_code)]
//! Integration tests for the Prometheus `/metrics` endpoint.
//!
//! The `/metrics` endpoint emits only a liveness gauge (`copypaste_relay_up`)
//! since CopyPaste-j21 stripped device/item counters from unauthenticated
//! endpoints to prevent operational metadata leaks.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[path = "../src/api/metrics.rs"]
mod metrics_handler;

// ---------------------------------------------------------------------------
// Test harness — minimal router with only /metrics wired
// ---------------------------------------------------------------------------

fn metrics_only_router() -> axum::Router {
    use axum::routing::get;
    axum::Router::new().route("/metrics", get(metrics_handler::handle))
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

/// The /metrics endpoint must respond 200 with Prometheus text/plain content-type
/// and emit only the liveness gauge (CopyPaste-j21: device/item counts stripped).
#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    let router = metrics_only_router();
    let (status, content_type, body) = fetch_metrics(&router).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("text/plain"),
        "expected Prometheus text/plain content-type, got {content_type:?}"
    );
    assert!(content_type.contains("version=0.0.4"));

    // The liveness gauge must be present.
    assert!(
        body.contains("# HELP copypaste_relay_up "),
        "missing HELP for copypaste_relay_up in body:\n{body}"
    );
    assert!(
        body.contains("# TYPE copypaste_relay_up gauge"),
        "missing TYPE for copypaste_relay_up in body:\n{body}"
    );
    assert!(
        body.contains("copypaste_relay_up 1"),
        "expected copypaste_relay_up 1 in body:\n{body}"
    );

    // Device/item counters must NOT appear in the unauthenticated response
    // (CopyPaste-j21 security hardening).
    assert!(
        !body.contains("copypaste_relay_items_total"),
        "items_total must not appear in unauthenticated /metrics: {body}"
    );
    assert!(
        !body.contains("copypaste_relay_evictions_total"),
        "evictions_total must not appear in unauthenticated /metrics: {body}"
    );
    assert!(
        !body.contains("copypaste_relay_active_devices"),
        "active_devices must not appear in unauthenticated /metrics: {body}"
    );
}

/// Verify the liveness gauge is stable across multiple requests (not affected
/// by store state).
#[tokio::test]
async fn liveness_gauge_is_always_one() {
    let router = metrics_only_router();

    // Multiple fetches must all return copypaste_relay_up 1.
    for _ in 0..3 {
        let (status, _, body) = fetch_metrics(&router).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("copypaste_relay_up 1"),
            "liveness gauge must always be 1, got body:\n{body}"
        );
    }
}
