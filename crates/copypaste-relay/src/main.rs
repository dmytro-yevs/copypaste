// M10 (audit 2026-05-27): enforce async-safe locking crate-wide.
// `std::sync::Mutex` guards must never be held across `.await` points —
// doing so can deadlock the tokio runtime. This lint is denied here
// rather than switching to `tokio::sync::Mutex` (12 lock sites, mostly
// short critical sections) so the smaller blast radius is preferred and
// any future violation fails the build.
#![deny(clippy::await_holding_lock)]

mod api;
mod auth;
mod config;
mod db;
mod error;
mod governor_cleanup;
mod middleware;
mod models;
mod quota;
mod routes;
mod state;
mod store;

use std::net::SocketAddr;
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
    // R1b: open the durable store and rehydrate any persisted state. With the
    // default `:memory:` db_path nothing is loaded (empty in-memory db); with a
    // file path the relay survives restart. A failure to open/load the store is
    // fatal — better to refuse to start than to silently serve an empty inbox
    // and lose every device's history.
    let relay_store = RelayStore::new_persistent(
        config.sync_ttl_secs,
        config.max_items_per_device,
        &config.db_path,
    )?;
    if config.db_path != db::IN_MEMORY_PATH {
        tracing::info!(db_path = %config.db_path, "relay persistence enabled (SQLite)");
    }
    let state = Arc::new(Mutex::new(relay_store));

    // Background TTL evictor — see ADR-009 (in-memory store + periodic prune).
    let _evictor =
        store::spawn_ttl_evictor(state.clone(), config.sync_ttl_secs, TTL_EVICTOR_TICK_SECS);

    let (app, retain_fns) =
        routes::relay_router(state, config.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Background governor cleanup — evict stale per-key rate-limit buckets
    // every 60 s to bound resident memory (one entry per distinct client IP /
    // device id accumulates without this).  The handle is kept alive for the
    // duration of the server; dropping it would cancel the task.
    let _governor_cleanup = governor_cleanup::spawn_cleanup_all(
        retain_fns,
        governor_cleanup::GOVERNOR_CLEANUP_TICK_SECS,
    );

    let addr = format!("{}:{}", config.bind_addr, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(
        "copypaste-relay listening on {addr} (ttl={}s, tick={}s)",
        config.sync_ttl_secs,
        TTL_EVICTOR_TICK_SECS
    );
    // `into_make_service_with_connect_info` is required so handlers like
    // `devices::register` can read the client's `SocketAddr` via the
    // `ConnectInfo` extractor — needed by the per-(ip, device) registration
    // rate limiter (security HIGH #5).
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
