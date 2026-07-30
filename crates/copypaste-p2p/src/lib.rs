//! Peer-to-peer clipboard sync over a LAN.
//!
//! # Shape of the thing
//!
//! Three pieces, deliberately separable:
//!
//! * [`transport`] — a mutually-authenticated, forward-secret channel built on
//!   the Noise framework (`snow`), pattern `NNpsk0`. The pairing token is the
//!   pre-shared key; possession of it *is* the authentication, so there are no
//!   certificates, no trust store and no pinning verifier to write.
//! * [`protocol`] and [`sync`] — what the two ends say to each other, and the
//!   last-write-wins merge that decides who is right.
//! * [`discovery`] — mDNS-SD, a convenience. Everything works with an explicit
//!   address; discovery only saves the user typing one.
//!
//! # Why no TLS and no PAKE
//!
//! v1 carried rustls, rcgen, a hand-written certificate-pinning verifier, two
//! hand-rolled DER parsers, and OPAQUE — an augmented PAKE — to solve this.
//!
//! The PAKE was answering a question we do not ask. A PAKE exists to make a
//! *low-entropy human secret* safe against an offline dictionary attack. Our
//! pairing token is 256 bits from the OS CSPRNG, so there is no dictionary; a
//! PSK handshake is exactly as strong and needs no registration step, no
//! password file to persist, and no second crypto stack.
//!
//! TLS went the same way. What we need is a channel between two devices that
//! already share a secret. Noise provides that as a primitive, with key
//! derivation, nonce management and rekeying handled by the library rather than
//! assembled by us.
//!
//! # What crosses the wire
//!
//! Item content travels as **plaintext inside the Noise channel**, and the
//! receiver re-encrypts it under its own local key.
//!
//! The alternative — forwarding the sender's ciphertext — cannot work: every
//! item is sealed with a key derived from the sending device's secret, and the
//! AEAD binds the item id, so the receiver could never open it. Re-encrypting
//! for each peer would mean sharing local storage keys between devices, which
//! is strictly worse than relying on the transport that already provides
//! confidentiality, integrity and forward secrecy.

#![forbid(unsafe_code)]

pub mod discovery;
pub mod peers;
pub mod protocol;
pub mod sync;
pub mod transport;

pub use peers::{Peer, PeerStore, PeerStoreError};
pub use protocol::{ItemSummary, SyncItem, SyncMessage, PROTOCOL_VERSION};
pub use sync::{merge_decision, MergeDecision, SyncOutcome, SyncStats};
pub use transport::{PairingToken, Session, TransportError};

/// TCP port the daemon listens on for peers.
///
/// Fixed rather than ephemeral so an explicit address is short to type. The
/// listener binds `0.0.0.0` but the channel refuses anyone without the PSK, so
/// an open port discloses only that CopyPaste is running.
pub const DEFAULT_PORT: u16 = 47_654;

/// mDNS service type used for discovery.
pub const SERVICE_TYPE: &str = "_copypaste._tcp.local.";
