//! `GET /metrics` — Prometheus text-format exposition endpoint.
//!
//! Returns only a liveness gauge (`copypaste_relay_up 1`). Device/item/
//! eviction counters are intentionally omitted from this unauthenticated
//! endpoint to prevent anonymous callers from learning how many devices are
//! registered or how much clipboard traffic the relay is processing
//! (CopyPaste-j21 security hardening). A future phase may gate the full
//! counter set behind an operator bearer token.
//!
//! The endpoint is intentionally rate-limit-exempt (same tier as
//! `/health`, `/stats`) so a scraping Prometheus does not have to share
//! the per-IP budget with real clients.

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

/// Prometheus text format `Content-Type` (version 0.0.4).
///
/// Spec: <https://prometheus.io/docs/instrumenting/exposition_formats/#text-based-format>
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// `GET /metrics` handler.
///
/// Returns a minimal liveness gauge only. Device count, item count, and
/// eviction count are not emitted here — those are operational metrics that
/// should not be visible to unauthenticated scrapers (CopyPaste-j21).
pub async fn handle() -> impl IntoResponse {
    // A single "up" gauge so a Prometheus scraper can confirm the relay is
    // reachable without the response leaking any device or item counters.
    let body = "# HELP copypaste_relay_up Whether the relay is up (1 = yes)\n\
                # TYPE copypaste_relay_up gauge\n\
                copypaste_relay_up 1\n";

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)],
        body,
    )
}
