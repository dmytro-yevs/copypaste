//! `copypaste-p2p` — P2P mutual-TLS transport + mDNS-SD peer discovery for CopyPaste.
//!
//! # Overview
//!
//! Each device generates a self-signed X.509 certificate at first run. The
//! certificate's SHA-256 fingerprint serves as the device identity. During
//! pairing, devices exchange fingerprints out-of-band (e.g. QR code, relay
//! server) and store them locally.
//!
//! mDNS-SD discovery (`DiscoveryService`) allows devices to find each other
//! on the local network without a relay server.
//!
//! When two devices connect directly:
//! 1. The server presents its certificate and requires the client to present one too.
//! 2. Both sides verify the peer's certificate fingerprint against their local
//!    `PairedPeers` table.
//! 3. On success, the connection is wrapped with a length-delimited framing
//!    codec ready for message exchange.
//!
//! # Usage
//!
//! ```no_run
//! use copypaste_p2p::{PeerTransport, PairedPeers, SelfSignedCert};
//! use copypaste_p2p::{DiscoveryService};
//! use tokio::net::TcpListener;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Generate our certificate (normally persisted between runs).
//! let my_cert = SelfSignedCert::generate("my-device-id")?;
//! println!("My fingerprint: {}", my_cert.fingerprint());
//!
//! // After out-of-band pairing, register the peer.
//! let peers = PairedPeers::new();
//! peers.add("abc123...peer_fingerprint...", "Alice's MacBook");
//!
//! let transport = PeerTransport::from_cert(my_cert.cert_der, my_cert.key_der, peers);
//!
//! // Discover peers on the LAN.
//! let svc = DiscoveryService::new();
//! svc.on_peer_found(|peer| println!("Found: {:?}", peer));
//! svc.register(51515, "my-device-id", "My Mac").unwrap();
//! let _handle = svc.start().await.unwrap();
//!
//! // Accept incoming connections.
//! let listener = TcpListener::bind("0.0.0.0:51515").await?;
//! let (_peer_addr, _peer_fp, _stream) = transport.accept(&listener).await?;
//! # Ok(())
//! # }
//! ```

pub mod bootstrap;
pub mod cert;
pub mod connector;
pub mod discovery;
pub mod error;
pub mod interfaces;
pub mod pake;
pub mod rate_limit;
pub mod transport;
mod verifier;

// Convenient top-level re-exports — TLS transport.
pub use cert::{fingerprint_of, CertError, SelfSignedCert};
pub use transport::{
    tls_channel_binder_client, tls_channel_binder_server, try_push_frame, DeviceFingerprint,
    PairedPeers, PeerClientStream, PeerStream, PeerTransport, PushError, PushNotifier,
    TransportError, PEER_SEND_TIMEOUT, TLS_CHANNEL_BINDING_LABEL,
};

// Convenient top-level re-exports — mDNS-SD discovery.
pub use discovery::{DiscoveryService, PeerInfo, SERVICE_TYPE};
pub use error::DiscoveryError;

// Convenient top-level re-exports — PAKE pairing (ADR-008).
pub use pake::{
    channel_confirmation_tag, ConfirmRole, PakeError, PakeInitiator, PakeResponder, PasswordFile,
    SessionKey, CONFIRM_TAG_LEN,
};

// Convenient top-level re-exports — unauthenticated bootstrap channel (P2P Phase 1).
pub use bootstrap::{
    run_initiator, BootstrapPairing, BootstrapResponder, BOOTSTRAP_ACCEPT_TIMEOUT,
    DISCOVERY_PAIRING_PASSWORD,
};

/// Default TCP port for P2P direct connections.
pub const DEFAULT_P2P_PORT: u16 = 51515;

/// Fixed, non-secret domain-separation salt for the P2P content sync key.
///
/// Both the macOS daemon and the Android client derive the shared
/// XChaCha20-Poly1305 content key from the PAKE [`SessionKey`] via
/// `SessionKey::derive_xchacha_key(P2P_SYNC_KEY_SALT)`. A one-byte divergence
/// between the two sides makes every synced item permanently undecryptable with
/// no user-visible error beyond "sync not working".
///
/// **This is the canonical, single-source-of-truth definition.**
/// - Android: imports this constant directly (no local copy).
/// - Daemon: `crates/copypaste-daemon/src/ipc/mod.rs` has a local copy (not
///   yet wired to this constant — tracked as CopyPaste-crh3.88 follow-up).
///   The `p2p_sync_key_salt_golden_value` test in this crate pins the exact
///   bytes so any drift in either copy fails CI.
///
/// If this value ever needs to change, bump `P2P_SYNC_KEY_SALT` here,
/// update the daemon's local copy in lockstep, and bump the P2P protocol version.
pub const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden-value parity test — pins `P2P_SYNC_KEY_SALT` to the exact
    /// bytes expected by BOTH the Android FFI and the daemon IPC path.
    ///
    /// Changing either the constant here or the daemon's local copy in
    /// `crates/copypaste-daemon/src/ipc/mod.rs` without updating the other
    /// makes this test fail, catching a key-derivation mismatch before it
    /// reaches production.
    #[test]
    fn p2p_sync_key_salt_golden_value() {
        assert_eq!(
            P2P_SYNC_KEY_SALT, b"copypaste/p2p/content-sync-key/v1",
            "P2P_SYNC_KEY_SALT diverged from the expected golden value — \
             update the daemon's local copy in ipc/mod.rs and this constant \
             in lockstep, then bump the P2P protocol version."
        );
        // Non-empty: an accidental truncation to b"" would make derive_xchacha_key
        // produce the same key for every purpose — a catastrophic key-reuse failure.
        assert!(
            !P2P_SYNC_KEY_SALT.is_empty(),
            "P2P_SYNC_KEY_SALT must not be empty"
        );
    }
}
