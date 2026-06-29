//! mDNS-SD peer discovery for CopyPaste.
//!
//! Registers own service under `_copypaste._tcp.local.` and browses for
//! other instances, emitting callbacks when peers appear or disappear.

mod browse;
mod registry;
mod service;
mod types;

pub use service::DiscoveryService;
pub use types::{PeerInfo, MDNS_REANNOUNCE_INTERVAL, PROTOCOL_VERSION_V1, SERVICE_TYPE};

use std::sync::{Mutex, MutexGuard, PoisonError};
use tracing::warn;

/// Lock a `Mutex` even if a previous holder panicked.
///
/// Poison-tolerance is required for callbacks that may panic: a panic in
/// `on_peer_found`/`on_peer_lost` user code would otherwise permanently
/// disable discovery for the rest of the process. We recover the inner
/// guard and log a warning so the issue surfaces in production telemetry.
#[inline]
pub(super) fn lock_safe<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock()
        .unwrap_or_else(|e: PoisonError<MutexGuard<'_, T>>| {
            warn!("recovering from poisoned mutex in discovery service");
            e.into_inner()
        })
}
