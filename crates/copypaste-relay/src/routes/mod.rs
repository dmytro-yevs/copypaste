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

/// Build the complete relay router with all routes wired.
pub fn relay_router(state: AppState, config: RelayConfig) -> Router {
    // Rate limit: 60 requests/minute per IP (1 req/s steady-state, burst of 20).
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(20)
            .finish()
            .expect("invalid governor configuration"),
    );

    Router::new()
        .route("/health", get(health::handle))
        .route("/stats", get(stats_handler))
        .route("/devices", get(list_devices_handler).post(devices::register))
        .route(
            "/devices/:device_id/items",
            get(items::pull).post(items::push),
        )
        .route(
            "/devices/:device_id/items/:item_id",
            delete(items::delete_item),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes + 4096,
        ))
        .layer(GovernorLayer {
            config: governor_conf,
        })
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
