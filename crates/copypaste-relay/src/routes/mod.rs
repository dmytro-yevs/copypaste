pub mod devices;
pub mod health;
pub mod items;

use std::sync::Arc;

use axum::routing::{delete, get, post};
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
        .route("/devices", post(devices::register))
        .route(
            "/devices/:device_id/items",
            get(items::poll).post(items::upload),
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
