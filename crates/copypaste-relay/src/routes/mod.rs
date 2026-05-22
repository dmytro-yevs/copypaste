pub mod devices;
pub mod health;
pub mod items;

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::Router;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;

use crate::config::RelayConfig;
use crate::state::AppState;

/// Build the complete relay router.
///
/// # Rate limiting
///
/// The router is split into sub-routers:
///
/// 1. **Exempt routes** (`/health`, `/stats`) — no rate limiting applied.
///    These are lightweight diagnostic endpoints that must remain available
///    even under load.
///
/// 2. **Rate-limited routes** — everything else:
///    - *Per-IP*: 200 requests/minute (3 req/s steady-state, burst 60).
///      Applied to all non-exempt routes.
///    - *Per-device*: 60 requests/minute (1 req/s steady-state, burst 20).
///      Applied specifically to device-scoped item routes.
///
/// Exceeding either limit returns **HTTP 429 Too Many Requests** with a
/// `Retry-After` header automatically set by `tower_governor`.
pub fn relay_router(state: AppState, config: RelayConfig) -> Router {
    // ---- Exempt routes (no rate limiting) ----------------------------------
    let exempt = Router::new()
        .route("/health", get(health::handle))
        .route("/stats", get(stats_handler))
        .with_state(state.clone());

    // ---- Per-IP rate limit layer (200 req/min) ------------------------------
    let per_ip_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(3)
            .burst_size(60)
            .finish()
            .expect("invalid per-IP governor configuration"),
    );

    // ---- Per-device rate limit layer (60 req/min) ---------------------------
    let per_device_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(20)
            .finish()
            .expect("invalid per-device governor configuration"),
    );

    // ---- Device-scoped item routes (per-device + per-IP limits) ------------
    let item_routes = Router::new()
        .route(
            "/devices/:device_id/items",
            get(items::pull).post(items::push),
        )
        .route(
            "/devices/:device_id/items/:item_id",
            delete(items::delete_item),
        )
        .with_state(state.clone())
        .layer(GovernorLayer { config: per_device_conf })
        .layer(GovernorLayer { config: per_ip_conf.clone() });

    // ---- Device registration + info routes (per-IP limit only) -------------
    let device_routes = Router::new()
        .route("/devices", get(list_devices_handler).post(devices::register))
        .route("/devices/:device_id", get(devices::get_device))
        .with_state(state)
        .layer(GovernorLayer { config: per_ip_conf });

    // ---- Merge all sub-routers + shared body-limit layer -------------------
    Router::new()
        .merge(exempt)
        .merge(item_routes)
        .merge(device_routes)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
}

async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.lock().unwrap();
    let (devices, items) = store.stats();
    axum::Json(serde_json::json!({
        "devices": devices,
        "total_items": items,
        "version": "2"
    }))
}

async fn list_devices_handler(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.lock().unwrap();
    let device_ids = store.list_devices();
    axum::Json(serde_json::json!({ "devices": device_ids }))
}
