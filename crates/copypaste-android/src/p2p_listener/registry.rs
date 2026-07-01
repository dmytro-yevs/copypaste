//! Process-global listener registry — the `u64`-handle idiom (mirrors
//! `DB_HANDLES` in `lib.rs`) — plus the live per-peer trust/key state
//! ([`PeerState`]) shared between the accept loop and
//! `update_p2p_listener_peers`.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, OnceLock};

use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::PairedPeers;

use crate::SyncedItem;

/// Per-peer PAKE session key, keyed by the peer's pinned cert fingerprint.
///
/// `session_key` is the 32-byte PAKE session key from the bootstrap pairing —
/// the SAME value `sync_with_peer` consumes. The shared content key is derived
/// from it per-peer (never a single global key), so a frame from peer A is only
/// ever decrypted with A's key.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after the call returns; never log it.
#[derive(Debug, Clone)]
pub struct PeerSessionKey {
    pub fingerprint: String,
    pub session_key: Vec<u8>,
}

/// FFI result of `start`: the registry handle plus the OS-assigned
/// port. When the caller passes `listen_port == 0` the kernel picks a free port
/// and `actual_port` reports it so Kotlin can advertise the real bind port.
#[derive(Debug, Clone)]
pub struct P2pListenerHandle {
    pub listener_id: u64,
    pub actual_port: u16,
}

/// Live, mutable per-peer trust + key state shared between the accept loop and
/// the `update_p2p_listener_peers` FFI. Held behind a `std::sync::Mutex`
/// (never across an `.await`) so a roster refresh is visible to the next
/// accepted connection without restarting the listener.
pub(super) struct PeerState {
    /// Pinned allowlist that the TLS verifier consults on every handshake.
    /// Shared by clone with the `PeerTransport` (interior `Arc<RwLock>`), so
    /// `add`/`remove` here take effect on subsequent handshakes.
    pub(super) peers: PairedPeers,
    /// The fingerprints currently pinned in `peers`. Tracked here because
    /// `PairedPeers` is not enumerable, so a roster refresh needs to know which
    /// previously-pinned fingerprints to `remove` before adding the new set.
    pub(super) allowed: Vec<String>,
    /// Current denylist (canonicalized compare happens in `is_fingerprint_revoked`).
    pub(super) revoked: Vec<String>,
    /// fingerprint → 32-byte PAKE session key for deriving that peer's content key.
    pub(super) session_keys: HashMap<String, Vec<u8>>,
}

/// Process-global handle to one running listener. Stored in [`LISTENER_REGISTRY`]
/// keyed by the `u64` handle the FFI hands back to Kotlin.
pub(super) struct ListenerHandle {
    /// Cancel token: `stop_p2p_listener` fires it and the accept loop's
    /// `select!` breaks, dropping the listener socket.
    pub(super) cancel: CancellationToken,
    /// The port the listener actually bound (resolved from `local_addr()`).
    #[allow(dead_code)]
    // retained for diagnostics / future status FFI; not read on the hot path.
    pub(super) actual_port: u16,
    /// Items decrypted from inbound frames, awaiting a `poll` drain.
    pub(super) received: Arc<Mutex<Vec<SyncedItem>>>,
    /// Live trust/key roster, refreshable via `update_p2p_listener_peers`.
    pub(super) peer_state: Arc<Mutex<PeerState>>,
}

/// Registry of running listeners. `OnceLock<Mutex<HashMap<…>>>` mirrors the
/// `DB_HANDLES` idiom in `lib.rs`.
pub(super) static LISTENER_REGISTRY: OnceLock<Mutex<HashMap<u64, ListenerHandle>>> =
    OnceLock::new();
/// Monotonic id source for new listener handles (starts at 1; 0 is never used
/// so a Kotlin caller can treat 0 as "no listener").
pub(super) static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn registry() -> &'static Mutex<HashMap<u64, ListenerHandle>> {
    LISTENER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
