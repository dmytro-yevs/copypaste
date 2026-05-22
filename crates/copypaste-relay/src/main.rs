mod auth;
mod config;
mod error;
mod models;
mod routes;
mod state;

use std::sync::{Arc, Mutex};

use config::RelayConfig;
use state::RelayStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = RelayConfig::from_env();
    let store = RelayStore::new(config.sync_ttl_secs);
    let state = Arc::new(Mutex::new(store));
    let app = routes::relay_router(state, config.clone());

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("copypaste-relay listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
