//! Background TTL eviction for the in-memory relay store.
//!
//! See [ADR-009](../../../docs/adr/ADR-009-relay-storage-choice.md) for the
//! decision rationale (in-memory `HashMap` over SQLite).
//!
//! ## Behaviour
//!
//! `spawn_ttl_evictor` starts a detached tokio task that, every
//! `tick_secs`, grabs the store lock, calls
//! [`RelayStore::prune_expired`](crate::state::RelayStore::prune_expired)
//! with the current wall-clock time, and releases the lock.
//!
//! The task uses `tokio::time::interval` so it is fully driven by Tokio's
//! virtual clock — tests can `tokio::time::pause()` and
//! `tokio::time::advance(...)` to deterministically trigger eviction.
//!
//! The task runs forever; cancellation is handled by dropping the
//! returned [`JoinHandle`] (or by process shutdown).

use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;

use crate::state::AppState;

/// Spawn a background tokio task that prunes expired sync items every
/// `tick_secs`.
///
/// Items are evicted when `inserted_at_unix + ttl_secs <= now_unix`. A
/// `ttl_secs` of 0 disables eviction (the task still ticks but does
/// nothing).
///
/// Returns the [`JoinHandle`] so the caller can keep or abort the task.
pub fn spawn_ttl_evictor(
    state: AppState,
    ttl_secs: u64,
    tick_secs: u64,
) -> JoinHandle<()> {
    let tick = std::time::Duration::from_secs(tick_secs.max(1));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(tick);
        // Skip the immediate first tick that `interval` fires.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let now_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let evicted = {
                let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
                store.prune_expired(now_unix, ttl_secs)
            };
            if evicted > 0 {
                tracing::debug!(
                    evicted,
                    ttl_secs,
                    "relay TTL evictor pruned expired items"
                );
            }
        }
    })
}
