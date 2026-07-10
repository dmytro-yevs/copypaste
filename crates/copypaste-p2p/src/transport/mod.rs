//! Mutual-TLS peer-to-peer transport.
//!
//! # Architecture
//!
//! ```text
//! PeerTransport::accept()   ← TcpListener (0.0.0.0:port)
//!                              TLS server (ClientAuth::Required)
//!                              fingerprint verified against PairedPeers
//!                              → Framed<TlsStream, LengthDelimitedCodec>
//!
//! PeerTransport::connect()  → TcpStream to peer addr
//!                              TLS client (presents own cert)
//!                              fingerprint verified against PairedPeers
//!                              → Framed<TlsStream, LengthDelimitedCodec>
//! ```
//!
//! Both sides require mutual TLS (`ClientAuth::Required` on the server,
//! custom verifier on both sides that checks the peer certificate fingerprint
//! against the `PairedPeers` table before allowing the handshake to complete).

mod connection;
mod peers;
mod push;
mod tls;

pub use connection::{
    PeerClientStream, PeerStream, PeerTransport, TransportError, CONNECT_RETRY_DELAY,
    MAX_CONNECT_ATTEMPTS, MAX_FRAME_BYTES, P2P_SNI_SENTINEL, TCP_CONNECT_TIMEOUT,
    TCP_KEEPALIVE_TIME, TLS_HANDSHAKE_TIMEOUT,
};
pub use peers::{DeviceFingerprint, PairedPeers, CERT_ROTATION_GRACE};
pub use push::{try_push_frame, PushError, PushNotifier, PEER_SEND_TIMEOUT};
pub use tls::{tls_channel_binder_client, tls_channel_binder_server, TLS_CHANNEL_BINDING_LABEL};
