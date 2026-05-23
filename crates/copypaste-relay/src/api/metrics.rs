//! `GET /metrics` — Prometheus text-format exposition endpoint.
//!
//! Emits three series:
//!
//! - `copypaste_relay_items_total` (counter) — total items ever stored
//!   (incremented in [`crate::state::RelayStore::push_item`]).
//! - `copypaste_relay_evictions_total` (counter) — total items evicted
//!   by TTL (incremented in [`crate::state::RelayStore::prune_expired`]).
//! - `copypaste_relay_active_devices` (gauge) — number of devices whose
//!   inbox currently holds at least one item.
//!
//! The endpoint is intentionally rate-limit-exempt (same tier as
//! `/health`, `/stats`) so a scraping Prometheus does not have to share
//! the per-IP budget with real clients.

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

use crate::state::AppState;

/// Prometheus text format `Content-Type` (version 0.0.4).
///
/// Spec: <https://prometheus.io/docs/instrumenting/exposition_formats/#text-based-format>
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// `GET /metrics` handler.
///
/// Snapshots the three metric values from `RelayStore` under a single
/// short-lived lock and formats them as Prometheus text.
pub async fn handle(State(state): State<AppState>) -> impl IntoResponse {
    let (items_total, evictions_total, active_devices) = {
        // Survive mutex poisoning (security INFO #21) — same pattern as
        // /health and /stats.
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.metrics_snapshot()
    };

    let body = format!(
        "# HELP copypaste_relay_items_total Total items ever stored\n\
         # TYPE copypaste_relay_items_total counter\n\
         copypaste_relay_items_total {items_total}\n\
         # HELP copypaste_relay_evictions_total Total items evicted by TTL\n\
         # TYPE copypaste_relay_evictions_total counter\n\
         copypaste_relay_evictions_total {evictions_total}\n\
         # HELP copypaste_relay_active_devices Number of devices with inbox entries\n\
         # TYPE copypaste_relay_active_devices gauge\n\
         copypaste_relay_active_devices {active_devices}\n"
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, PROMETHEUS_CONTENT_TYPE)],
        body,
    )
}
