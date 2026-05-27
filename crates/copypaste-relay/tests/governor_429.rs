//! Regression test: an overloaded rate-limited route must return HTTP 429.
//!
//! Builds a minimal router protected by a `GovernorLayer` with the tightest
//! possible configuration (burst=1, 1 replenishment/s) using the
//! `GlobalKeyExtractor` (all requests share one bucket, no IP required) so
//! the test is self-contained — no real TCP connection or `ConnectInfo`
//! extension needed.
//!
//! Pattern: `tower::ServiceExt::oneshot` (same as `tests/integration.rs`).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower::ServiceExt;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::GlobalKeyExtractor, GovernorLayer,
};

fn rate_limited_router() -> Router {
    // 1 request per second, burst of 1 — the second back-to-back request
    // must be rejected.
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(GlobalKeyExtractor)
            .per_second(1)
            .burst_size(1)
            .finish()
            // burst_size=1 and period=1s are both non-zero, so this is infallible.
            .expect("tight governor config must be valid"),
    );

    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .layer(GovernorLayer::new(config))
}

#[tokio::test]
async fn second_request_returns_429_when_burst_exhausted() {
    let app = rate_limited_router();

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("first request must complete");

    assert_eq!(
        first.status(),
        StatusCode::OK,
        "first request must succeed (burst not yet exhausted)"
    );

    let second = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("second request must complete");

    assert_eq!(
        second.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "second back-to-back request must be rate-limited (HTTP 429)"
    );

    // tower_governor 0.8 always sets Retry-After on 429 responses.
    assert!(
        second.headers().contains_key("retry-after"),
        "HTTP 429 must include a Retry-After header"
    );
}
