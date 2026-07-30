//! Peer-to-peer clipboard sync over a LAN.
//!
//! Authentication is possession of the pairing token: it is the pre-shared key
//! of a Noise `NNpsk0` channel ([`transport`]), so there are no certificates,
//! no trust store and no pinning verifier — and no PAKE, because a PAKE
//! protects a *low-entropy human secret* from an offline dictionary attack and
//! a 256-bit CSPRNG token has no dictionary. v1 spent rustls, rcgen, a
//! hand-written pinning verifier, two hand-rolled DER parsers and OPAQUE here;
//! the full argument is in `transport/handshake.rs`.
//!
//! # What crosses the wire
//!
//! Item content travels as **plaintext inside the Noise channel**, and the
//! receiver re-encrypts it under its own local key. Forwarding the sender's
//! ciphertext cannot work: every item is sealed with a key derived from the
//! sending device's secret and the AEAD binds the item id, so the receiver
//! could never open it — and re-encrypting per peer would mean sharing local
//! storage keys between devices.

#![forbid(unsafe_code)]

pub mod discovery;
pub mod netif;
pub mod node;
pub mod peers;
pub mod protocol;
pub mod sync;
pub mod transport;

pub use node::{Node, NodeError};
pub use peers::{Peer, PeerStore, PeerStoreError, RevokedDevice, PAIRING_CODE_TTL};
pub use protocol::{ItemSummary, SyncItem, SyncMessage, PROTOCOL_VERSION};
pub use sync::{merge_decision, MergeDecision, SyncOutcome, SyncStats};
pub use transport::{PairingToken, Session, TransportError};

/// TCP port the daemon listens on for peers.
///
/// Fixed rather than ephemeral so an explicit address is short to type. The
/// channel refuses anyone without the PSK, so an open port discloses only that
/// CopyPaste is running.
pub const DEFAULT_PORT: u16 = 47_654;

/// mDNS service type used for discovery.
pub const SERVICE_TYPE: &str = "_copypaste._tcp.local.";

/// Milliseconds since the Unix epoch.
///
/// Local because this crate does not depend on `copypaste-core`, where the
/// shared helper lives — but one copy, not one per module: discovery, sync and
/// the pairing deadline had grown their own. A clock before the epoch reads as
/// 0, which loses every comparison, and losing is the safe direction for all
/// three.
pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
