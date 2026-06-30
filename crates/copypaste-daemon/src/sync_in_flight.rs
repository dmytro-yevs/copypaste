//! RAII guard for the `sync_in_flight` flag (CopyPaste-1jms.22).
//!
//! ## Purpose
//!
//! `SyncBadgeState::Syncing` requires the daemon to signal when a sync
//! round-trip is actively in progress. The `IpcServer::get_sync_status` handler
//! reads a shared `Arc<AtomicBool>` (`sync_in_flight`) and passes it as
//! `in_flight` to [`copypaste_ipc::compute_sync_badge_state_with_inflight`].
//!
//! Each sync path (cloud poll, cloud push, relay receive, relay push, P2P
//! handshake) holds a [`crate::sync_in_flight::SyncInFlightGuard`] for the duration of its active
//! network exchange.  On construction the guard sets the flag to `true`; on
//! `Drop` it resets to `false`.  Using a guard (instead of manual `store(true)`
//! / `store(false)` pairs) guarantees that **every exit path** — normal return,
//! early return via `?`, or an async task cancellation — resets the flag,
//! preventing a stuck-"syncing" badge regression.
//!
//! ## Placement
//!
//! The guard covers only the ACTIVE network exchange — not idle poll waits or
//! backoff sleeps. Loop-level wait periods must NOT be counted as in-flight:
//! `SyncInFlightGuard` is created just before the network call and dropped as
//! soon as the round-trip completes or fails.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// RAII guard that holds the `sync_in_flight` flag `true` for its lifetime.
///
/// Create one just before a sync round-trip begins; let it drop when the
/// round-trip ends (success, error, or early return via `?`).  `Drop` always
/// resets the flag to `false`, guaranteeing no stuck-"syncing" badge regardless
/// of how the scope exits.
///
/// # Example
///
/// ```ignore
/// let _guard = SyncInFlightGuard::new(Arc::clone(&self.sync_in_flight));
/// let result = client.get(&url).send().await?; // flag is true here
/// // _guard dropped here → flag reset to false
/// ```
pub struct SyncInFlightGuard {
    flag: Arc<AtomicBool>,
}

impl SyncInFlightGuard {
    /// Create the guard and immediately set `flag` to `true`.
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Release);
        Self { flag }
    }
}

impl Drop for SyncInFlightGuard {
    fn drop(&mut self) {
        // Release ordering: the `true`→`false` transition is visible to any
        // reader that uses `Acquire` on the same flag (e.g. `get_sync_status`).
        self.flag.store(false, Ordering::Release);
    }
}
