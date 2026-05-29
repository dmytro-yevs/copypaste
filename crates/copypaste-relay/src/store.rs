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

/// How long after registration a device with a *persistently empty* inbox is
/// considered inactive and its record (plus its id-counter map entry) reclaimed
/// by the evictor (H1). Set to 30 days: long enough that a device that simply
/// hasn't received any fan-out yet is never evicted out from under an active
/// user, short enough that abandoned registrations don't leak forever. A device
/// whose inbox has *any* item is always retained regardless of age (see
/// [`RelayStore::cleanup_inactive_devices`]).
const DEVICE_INACTIVE_THRESHOLD_SECS: u64 = 30 * 24 * 3600;

/// Spawn a background tokio task that prunes expired sync items and reclaims
/// inactive device records every `tick_secs`.
///
/// Each tick:
///  1. evicts sync items whose `inserted_at_unix + ttl_secs <= now_unix`
///     (a `ttl_secs` of 0 disables item eviction — the task still ticks);
///  2. removes device records that registered ≥ [`DEVICE_INACTIVE_THRESHOLD_SECS`]
///     ago and have an empty inbox, reclaiming their `devices` /
///     `next_sync_id_per_device` entries (H1 — `cleanup_inactive_devices` was
///     previously never called, so those maps grew without bound).
///
/// Returns the [`JoinHandle`] so the caller can keep or abort the task.
pub fn spawn_ttl_evictor(state: AppState, ttl_secs: u64, tick_secs: u64) -> JoinHandle<()> {
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
            let (evicted, reclaimed) = {
                let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
                let evicted = store.prune_expired(now_unix, ttl_secs);
                let reclaimed = store.cleanup_inactive_devices(DEVICE_INACTIVE_THRESHOLD_SECS);
                (evicted, reclaimed)
            };
            if evicted > 0 {
                tracing::debug!(evicted, ttl_secs, "relay TTL evictor pruned expired items");
            }
            if reclaimed > 0 {
                tracing::debug!(reclaimed, "relay evictor reclaimed inactive device records");
            }
        }
    })
}
