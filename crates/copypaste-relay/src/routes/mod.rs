pub mod devices;
pub mod health;
pub mod items;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::config::RelayConfig;
use crate::state::AppState;

/// Build the complete relay router with all routes wired.
pub fn relay_router(state: AppState, config: RelayConfig) -> Router {
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
}
