mod auth;
mod config;
mod error;
mod middleware;
mod models;
mod quota;
mod routes;
mod state;
mod store;

use std::sync::{Arc, Mutex};

use config::RelayConfig;
use state::RelayStore;

/// How often the TTL evictor runs. Kept short relative to typical TTLs
/// (default 86400 s) so eviction is at most ~1 minute stale.
const TTL_EVICTOR_TICK_SECS: u64 = 60;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = RelayConfig::from_env();
    let relay_store = RelayStore::new(config.sync_ttl_secs);
    let state = Arc::new(Mutex::new(relay_store));

    // Background TTL evictor — see ADR-009 (in-memory store + periodic prune).
    let _evictor = store::spawn_ttl_evictor(
        state.clone(),
        config.sync_ttl_secs,
        TTL_EVICTOR_TICK_SECS,
    );

    let app = routes::relay_router(state, config.clone());

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(
        "copypaste-relay listening on {addr} (ttl={}s, tick={}s)",
        config.sync_ttl_secs,
        TTL_EVICTOR_TICK_SECS
    );
    axum::serve(listener, app).await?;

    Ok(())
}
