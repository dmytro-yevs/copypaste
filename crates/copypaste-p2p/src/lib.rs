//! Peer-to-peer clipboard sync over a LAN.
//!
//! Authentication is possession of the pairing token: it is the pre-shared key
//! of a Noise `NNpsk0` channel ([`transport`]), so there are no certificates,
//! no trust store and no pinning verifier. New invitations use an eight-symbol
//! one-time code with SPAKE2 to authenticate the channel that transfers the
//! full random token. Both devices must still confirm the handshake-bound SAS.
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

pub mod device_profile;
pub mod discovery;
pub mod netif;
pub mod node;
pub mod pairing_link;
pub mod peers;
pub mod protocol;
pub mod sync;
pub mod transport;

pub use copypaste_ipc::{DeviceClass, DevicePlatform};
#[cfg(target_os = "android")]
pub use device_profile::AndroidHardwareProfile;
pub use device_profile::{AuthenticatedDeviceProfile, DeviceProfile};
pub use node::{
    AuthenticatedReachability, Node, NodeError, PairingInvite, PairingPeer, PairingPhase,
    PairingRole, PairingStatus, ProbeState, PAIRING_CONFIRM_TIMEOUT, PAIRING_INVITE_TTL,
    PROBE_FAILURE_FRESH_MS, PROBE_FRESH_MS,
};
pub use pairing_link::{PairingLink, PairingLinkError, PAIRING_URI_HOST, PAIRING_URI_SCHEME};
pub use peers::{Peer, PeerStore, PeerStoreError, RevokedDevice};
pub use protocol::{ItemSummary, SyncItem, SyncMessage, PROTOCOL_VERSION};
pub use sync::{merge_decision, MergeDecision, SyncOutcome, SyncStats};
pub use transport::PairingCode;
pub use transport::{PairingToken, PskCandidate, Session, TransportError};

/// TCP port the daemon listens on for peers.
///
/// Fixed rather than ephemeral so an explicit address is short to type. The
/// channel refuses anyone without the PSK, so an open port discloses only that
/// CopyPaste is running.
pub const DEFAULT_PORT: u16 = 47_654;

/// mDNS service type used for discovery.
pub const SERVICE_TYPE: &str = "_copypaste._tcp.local.";

pub(crate) use copypaste_clock::now_ms;
