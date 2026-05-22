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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::cert::{fingerprint_of, SelfSignedCert};
use crate::verifier::PeerCertVerifier;

/// Opaque device identity — the SHA-256 fingerprint of the device's TLS cert
/// encoded as lowercase hex.
pub type DeviceFingerprint = String;

/// Map of known paired peers: their fingerprint → optional display name.
///
/// Before the TLS handshake, the transport checks that the peer's certificate
/// fingerprint is in this map. Connections from unknown fingerprints are
/// rejected.
#[derive(Clone, Default, Debug)]
pub struct PairedPeers {
    inner: HashMap<DeviceFingerprint, String>,
}

impl PairedPeers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a paired peer. `fingerprint` is hex(SHA-256(cert_der)).
    pub fn add(&mut self, fingerprint: impl Into<String>, display_name: impl Into<String>) {
        self.inner.insert(fingerprint.into(), display_name.into());
    }

    /// Returns `true` if `fingerprint` belongs to a known paired peer.
    pub fn is_known(&self, fingerprint: &str) -> bool {
        self.inner.contains_key(fingerprint)
    }
}

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
}

/// A framed async I/O stream after a successful mutual-TLS handshake.
///
/// Messages are length-delimited (4-byte big-endian prefix).
pub type PeerStream = Framed<tokio_rustls::server::TlsStream<TcpStream>, LengthDelimitedCodec>;

/// Same as `PeerStream` but for the client side of the connection.
pub type PeerClientStream =
    Framed<tokio_rustls::client::TlsStream<TcpStream>, LengthDelimitedCodec>;

/// The main entry point for P2P TLS connections.
///
/// Holds the device's own certificate/key and the set of paired peers. Both
/// `accept()` and `connect()` verify the peer fingerprint before returning a
/// usable stream.
pub struct PeerTransport {
    /// Our own certificate (DER).
    own_cert_der: Vec<u8>,
    /// Our own private key (DER).
    own_key_der: Vec<u8>,
    /// Our own fingerprint (hex SHA-256 of cert DER).
    own_fingerprint: DeviceFingerprint,
    /// Known paired peers.
    peers: Arc<PairedPeers>,
}

impl PeerTransport {
    /// Create a new transport using a freshly-generated self-signed certificate.
    pub fn new_with_generated_cert(
        device_id: &str,
        peers: PairedPeers,
    ) -> Result<Self, TransportError> {
        let cert = SelfSignedCert::generate(device_id)?;
        Ok(Self::from_cert(cert.cert_der, cert.key_der, peers))
    }

    /// Create a transport from existing DER-encoded certificate and private key.
    pub fn from_cert(
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
        peers: PairedPeers,
    ) -> Self {
        let own_fingerprint = fingerprint_of(&cert_der);
        Self {
            own_cert_der: cert_der,
            own_key_der: key_der,
            own_fingerprint,
            peers: Arc::new(peers),
        }
    }

    /// Returns our device's certificate fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.own_fingerprint
    }

    /// Bind a TCP listener on `addr` and wait for one incoming mutual-TLS connection.
    ///
    /// On success, returns the remote `SocketAddr` and a framed stream ready for
    /// message exchange.
    pub async fn accept(
        &self,
        listener: &TcpListener,
    ) -> Result<(SocketAddr, PeerStream), TransportError> {
        let server_config = self.build_server_config()?;
        let acceptor = TlsAcceptor::from(Arc::new(server_config));

        let (tcp_stream, peer_addr) = listener.accept().await?;
        tracing::debug!(peer_addr = %peer_addr, "incoming TCP connection");

        let tls_stream = acceptor.accept(tcp_stream).await?;

        // Extract and verify the peer's certificate fingerprint.
        let peer_fp = peer_fingerprint_server(&tls_stream)?;
        tracing::debug!(peer_fingerprint = %peer_fp, "peer cert fingerprint");

        if !self.peers.is_known(&peer_fp) {
            return Err(TransportError::UnknownPeer(peer_fp));
        }
        tracing::info!(peer_addr = %peer_addr, peer_fingerprint = %peer_fp, "peer authenticated");

        let framed = Framed::new(tls_stream, LengthDelimitedCodec::new());
        Ok((peer_addr, framed))
    }

    /// Connect to a peer at `addr` using mutual TLS.
    ///
    /// On success, returns a framed stream ready for message exchange.
    pub async fn connect(
        &self,
        addr: SocketAddr,
        expected_fingerprint: &str,
    ) -> Result<PeerClientStream, TransportError> {
        let client_config = self.build_client_config(expected_fingerprint)?;
        let connector = TlsConnector::from(Arc::new(client_config));

        let tcp_stream = TcpStream::connect(addr).await?;
        tracing::debug!(peer_addr = %addr, "TCP connection established");

        // rustls requires a ServerName even for mutual-TLS peer-to-peer.
        // We use a fixed placeholder since identity is verified by fingerprint.
        let server_name = ServerName::try_from("copypaste.peer")
            .expect("static server name is always valid");

        let tls_stream = connector.connect(server_name, tcp_stream).await?;
        tracing::info!(peer_addr = %addr, expected_fingerprint = %expected_fingerprint, "peer authenticated");

        let framed = Framed::new(tls_stream, LengthDelimitedCodec::new());
        Ok(framed)
    }

    // ---- private helpers ----

    fn build_server_config(&self) -> Result<ServerConfig, TransportError> {
        let cert = CertificateDer::from(self.own_cert_der.clone());
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(self.own_key_der.clone());
        let private_key = PrivateKeyDer::Pkcs8(key);

        let verifier = PeerCertVerifier::new(Arc::clone(&self.peers));

        let config = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(verifier))
            .with_single_cert(vec![cert], private_key)
            .map_err(TransportError::TlsConfig)?;

        Ok(config)
    }

    fn build_client_config(
        &self,
        expected_fingerprint: &str,
    ) -> Result<ClientConfig, TransportError> {
        let cert = CertificateDer::from(self.own_cert_der.clone());
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(self.own_key_der.clone());
        let private_key = PrivateKeyDer::Pkcs8(key);

        // We use a custom server verifier that only checks the fingerprint.
        let verifier =
            PeerCertVerifier::new_with_expected(Arc::clone(&self.peers), expected_fingerprint);

        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_client_auth_cert(vec![cert], private_key)
            .map_err(TransportError::TlsConfig)?;

        Ok(config)
    }
}

// ---- helper: extract fingerprint from completed server-side TLS stream ----

fn peer_fingerprint_server(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<String, TransportError> {
    let (_, server_conn) = stream.get_ref();
    let certs = server_conn
        .peer_certificates()
        .ok_or(TransportError::NoPeerCert)?;
    let first = certs.first().ok_or(TransportError::NoPeerCert)?;
    Ok(fingerprint_of(first.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Spin up a server and client in-process over TCP loopback, perform a
    /// mutual-TLS handshake, and verify both sides get a usable stream.
    #[tokio::test]
    async fn mutual_tls_loopback_handshake_succeeds() {
        // Generate two device certs.
        let server_cert = SelfSignedCert::generate("server-device").unwrap();
        let client_cert = SelfSignedCert::generate("client-device").unwrap();

        let server_fp = server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        // Server knows the client; client knows the server.
        let mut server_peers = PairedPeers::new();
        server_peers.add(client_fp.clone(), "client-device");

        let mut client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server-device");

        let server_transport = PeerTransport::from_cert(
            server_cert.cert_der,
            server_cert.key_der,
            server_peers,
        );
        let client_transport = PeerTransport::from_cert(
            client_cert.cert_der,
            client_cert.key_der,
            client_peers,
        );

        // Bind on a random loopback port.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Run server and client concurrently.
        let server_fut = server_transport.accept(&listener);
        let client_fut = client_transport.connect(addr, &server_fp);

        let (server_result, client_result) = tokio::join!(server_fut, client_fut);

        let (_peer_addr, _server_stream) = server_result.expect("server accept must succeed");
        let _client_stream = client_result.expect("client connect must succeed");
    }

    /// An unknown client cert must be rejected by the server.
    #[tokio::test]
    async fn unknown_peer_cert_is_rejected() {
        let server_cert = SelfSignedCert::generate("server-device").unwrap();
        let unknown_cert = SelfSignedCert::generate("unknown-device").unwrap();

        let server_fp = server_cert.fingerprint();

        // Server knows nobody.
        let server_peers = PairedPeers::new();

        // Client pretends to know the server, but the server won't accept the client.
        let mut client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server-device");

        let server_transport =
            PeerTransport::from_cert(server_cert.cert_der, server_cert.key_der, server_peers);
        let client_transport = PeerTransport::from_cert(
            unknown_cert.cert_der,
            unknown_cert.key_der,
            client_peers,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_fut = server_transport.accept(&listener);
        let client_fut = client_transport.connect(addr, &server_fp);

        let (server_result, _client_result) = tokio::join!(server_fut, client_fut);

        // The server must reject the unknown client.
        assert!(server_result.is_err(), "server must reject unknown peer");
    }
}
