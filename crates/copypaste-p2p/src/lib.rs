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
    TransportError, TLS_CHANNEL_BINDING_LABEL, PEER_SEND_TIMEOUT,
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
