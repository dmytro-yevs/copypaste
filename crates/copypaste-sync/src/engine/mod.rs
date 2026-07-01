/// P2P sync engine.
///
/// **NOT on the daemon production path** (CopyPaste-j6r/ayvs): the live daemon
/// does not instantiate `SyncEngine`. P2P sync in the daemon runs through
/// `copypaste-daemon::sync_orch` (which calls [`crate::merge::resolve`]
/// directly), and cloud/relay reuse the same [`crate::merge::remote_wins`]
/// total order. This engine + its HELLO/HAVE/WANT/ITEMS/DONE protocol are kept
/// for completeness and tests; see the crate-root docs before wiring them in.
///
/// `SyncEngine` orchestrates the item exchange loop between two peers over a
/// bidirectional byte stream (typically a TLS TCP socket).  It is intentionally
/// transport-agnostic: callers pass in an `AsyncRead + AsyncWrite` and the
/// engine drives the protocol to completion.
///
/// # Protocol overview
///
/// Both peers play symmetric roles after the initial HELLO handshake:
///
/// ```text
/// A ──HELLO──▶ B          (A sends first, B replies)
/// A ◀──HELLO── B
/// A ──HAVE───▶ B          (announce which item IDs each side has)
/// A ◀──HAVE─── B
/// A ──WANT───▶ B          (request what we don't have)
/// A ◀──WANT─── B
/// A ──ITEMS──▶ B          (send what the peer requested)
/// A ◀──ITEMS── B
/// A ──DONE───▶ B
/// A ◀──DONE─── B
/// ```
///
/// After both DONE messages are exchanged the connection can be dropped.
use std::collections::HashMap;

use crate::clock::LamportClock;

mod bounds;
mod error;
mod framing;
mod session;
#[cfg(test)]
mod tests;

pub use bounds::{MAX_LAMPORT_SKEW, MAX_WALL_TIME_SKEW_MS};
pub use error::SyncError;
pub use framing::MAX_FRAME_BYTES;
// `pub(crate)` re-exports so `engine::tests` reaches the framing internals via
// `use super::*` — crate-internal only, not a public-API widening. Gated to
// test builds since non-test code imports these directly from `framing`.
#[cfg(test)]
pub(crate) use framing::{recv_message, send_message, MAX_FRAME_SIZE};

/// Outcome of a completed sync session.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SyncResult {
    /// Items accepted from the remote peer (after LWW merge).
    pub items_received: usize,
    /// Items sent to the remote peer.
    pub items_sent: usize,
    /// Items that were already present locally and not replaced (LWW kept local).
    pub items_skipped: usize,
}

/// State tracked for a known peer across sessions.
#[derive(Debug, Clone, Default)]
pub struct PeerState {
    /// Last known Lamport clock value reported by this peer.
    pub last_clock: u64,
}

/// The sync engine for a single device.
///
/// Holds the device identity, its Lamport clock, and known peer clock values.
/// Multiple sync sessions can be driven sequentially via `run_session`.
pub struct SyncEngine {
    /// This device's UUID (used as `origin_device_id` when sending items).
    pub device_id: String,
    /// Logical clock maintained across sessions.
    pub clock: LamportClock,
    /// Per-peer clock bookkeeping (persisted across sessions externally).
    pub peer_clocks: HashMap<String, PeerState>,
}

impl SyncEngine {
    /// Create a new engine for the given device.
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
            clock: LamportClock::new(),
            peer_clocks: HashMap::new(),
        }
    }

    /// Restore an engine from persisted state.
    pub fn with_state(
        device_id: impl Into<String>,
        clock_value: u64,
        peer_clocks: HashMap<String, PeerState>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            clock: LamportClock::from_value(clock_value),
            peer_clocks,
        }
    }

    /// Record a local write event, advancing the Lamport clock.
    ///
    /// Returns the new clock value to be stamped on the written item.
    pub fn on_local_write(&mut self) -> i64 {
        self.clock.tick() as i64
    }
}
