//! `TransportError` + the framed-stream type aliases used by both sides of a
//! mutual-TLS P2P connection.

use thiserror::Error;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::config::TLS_HANDSHAKE_TIMEOUT;

/// Errors from the P2P transport.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS configuration error: {0}")]
    TlsConfig(#[from] rustls::Error),

    #[error("Certificate error: {0}")]
    Cert(#[from] crate::cert::CertError),

    #[error("Unknown peer: fingerprint '{0}' not in paired peers")]
    UnknownPeer(String),

    #[error("Peer presented no certificate")]
    NoPeerCert,

    #[error("Invalid private key encoding")]
    InvalidKey,

    #[error("TLS handshake timed out after {:?}", TLS_HANDSHAKE_TIMEOUT)]
    HandshakeTimeout,
}

/// A framed async I/O stream after a successful mutual-TLS handshake.
///
/// Messages are length-delimited (4-byte big-endian prefix).
pub type PeerStream = Framed<tokio_rustls::server::TlsStream<TcpStream>, LengthDelimitedCodec>;

/// Same as `PeerStream` but for the client side of the connection.
pub type PeerClientStream =
    Framed<tokio_rustls::client::TlsStream<TcpStream>, LengthDelimitedCodec>;
