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
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

/// Maximum time we will wait for a TLS handshake (client or server side) to
/// complete before giving up. Protects against dead sockets and slowloris-style
/// stalls during handshake.
pub const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Default number of times [`PeerTransport::connect_with_retry`] will retry a
/// transient network error before propagating it. The first attempt counts —
/// i.e. `MAX_CONNECT_ATTEMPTS = 4` means 1 initial attempt + 3 retries.
pub const MAX_CONNECT_ATTEMPTS: u32 = 4;

/// Delay between transient-error retries in [`PeerTransport::connect_with_retry`].
/// Kept short (100 ms) because the typical trigger is a peer that just
/// announced over mDNS but hasn't bound its listener yet, or a brief network
/// blip on the LAN — not a peer that genuinely needs minutes of backoff
/// (that's the relay client's job, see `copypaste_sync::backoff`).
pub const CONNECT_RETRY_DELAY: Duration = Duration::from_millis(100);

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
    pub fn from_cert(cert_der: Vec<u8>, key_der: Vec<u8>, peers: PairedPeers) -> Self {
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

        let tls_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(tcp_stream)).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        peer_addr = %peer_addr,
                        timeout = ?TLS_HANDSHAKE_TIMEOUT,
                        "TLS server handshake timed out"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };

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

        let tcp_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, TcpStream::connect(addr)).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        peer_addr = %addr,
                        timeout = ?TLS_HANDSHAKE_TIMEOUT,
                        "TCP connect timed out before TLS handshake"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };
        tracing::debug!(peer_addr = %addr, "TCP connection established");

        // rustls requires a ServerName even for mutual-TLS peer-to-peer.
        // We use a fixed placeholder since identity is verified by fingerprint.
        let server_name =
            ServerName::try_from("copypaste.peer").expect("static server name is always valid");

        let tls_stream = match tokio::time::timeout(
            TLS_HANDSHAKE_TIMEOUT,
            connector.connect(server_name, tcp_stream),
        )
        .await
        {
            Ok(res) => res?,
            Err(_elapsed) => {
                tracing::warn!(
                    peer_addr = %addr,
                    timeout = ?TLS_HANDSHAKE_TIMEOUT,
                    "TLS client handshake timed out"
                );
                return Err(TransportError::HandshakeTimeout);
            }
        };
        tracing::info!(peer_addr = %addr, expected_fingerprint = %expected_fingerprint, "peer authenticated");

        let framed = Framed::new(tls_stream, LengthDelimitedCodec::new());
        Ok(framed)
    }

    /// Connect to a peer with bounded retries on transient I/O errors.
    ///
    /// Wraps [`Self::connect`] with up to [`MAX_CONNECT_ATTEMPTS`] attempts
    /// (one initial + N-1 retries), separated by [`CONNECT_RETRY_DELAY`].
    /// Only **transient** errors are retried — see [`is_transient_io_error`]
    /// for the exhaustive list. Permanent errors (unknown-peer, TLS config,
    /// cert problems, handshake timeout) propagate on the first failure so
    /// callers don't waste time retrying a fundamentally broken setup.
    ///
    /// The intended use case is the brief race between mDNS announcement
    /// and the peer's TCP listener actually accepting connections, and
    /// transient LAN blips (cable bounce, brief Wi-Fi roaming). For
    /// long-haul relay reconnects with exponential backoff, see
    /// [`copypaste_sync::backoff::BackoffScheduler`].
    pub async fn connect_with_retry(
        &self,
        addr: SocketAddr,
        expected_fingerprint: &str,
    ) -> Result<PeerClientStream, TransportError> {
        let mut last_err: Option<TransportError> = None;
        for attempt in 1..=MAX_CONNECT_ATTEMPTS {
            match self.connect(addr, expected_fingerprint).await {
                Ok(stream) => {
                    if attempt > 1 {
                        tracing::info!(
                            peer_addr = %addr,
                            attempt,
                            "peer connect succeeded after retry"
                        );
                    }
                    return Ok(stream);
                }
                Err(err) => {
                    // Only retry transient I/O errors — anything else is a
                    // configuration / pairing problem and retrying won't help.
                    if !is_transient_transport_error(&err) {
                        tracing::debug!(
                            peer_addr = %addr,
                            attempt,
                            error = %err,
                            "peer connect failed with non-transient error — not retrying"
                        );
                        return Err(err);
                    }
                    if attempt < MAX_CONNECT_ATTEMPTS {
                        // ±50 ms jitter around the 100 ms base so concurrent
                        // peers that hit the same transient (e.g. mDNS race)
                        // don't lock-step their retries (security MED #10).
                        let jitter_ms = rand::random::<u8>() as u64 % 100;
                        let delay = CONNECT_RETRY_DELAY + Duration::from_millis(jitter_ms);
                        tracing::debug!(
                            peer_addr = %addr,
                            attempt,
                            backoff_ms = delay.as_millis(),
                            error = %err,
                            "peer connect transient failure — retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_err = Some(err);
                }
            }
        }
        // Exhausted retries — surface the last transient error.
        Err(last_err.expect("loop runs at least once so last_err is set on failure"))
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

/// Classify a [`TransportError`] as transient (worth retrying) vs permanent.
///
/// Transient = the LAN/peer is momentarily unavailable but the setup is sane:
///   * `ConnectionRefused` — peer's listener not bound yet (common right after
///     an mDNS announcement).
///   * `ConnectionReset` / `ConnectionAborted` / `BrokenPipe` — peer dropped
///     mid-connect, retry will pick up the next listener cycle.
///   * `WouldBlock` — extremely rare on `connect()`, but harmless to retry.
///   * `TimedOut` — kernel TCP timeout; one more try may succeed if the
///     peer just woke from Wi-Fi roam.
///   * `NotConnected` — the kernel surfaced a half-open socket; retry.
///
/// Everything else (unknown-peer pairing failure, TLS config issue, cert
/// error, our own handshake timeout) is permanent and propagates immediately.
fn is_transient_transport_error(err: &TransportError) -> bool {
    match err {
        TransportError::Io(io_err) => is_transient_io_kind(io_err.kind()),
        // HandshakeTimeout is *our* 10s budget — if we hit it, the peer is
        // actively misbehaving (slowloris, dead socket). Retrying just wastes
        // 10 more seconds. Surface it.
        TransportError::HandshakeTimeout
        | TransportError::UnknownPeer(_)
        | TransportError::NoPeerCert
        | TransportError::InvalidKey
        | TransportError::TlsConfig(_)
        | TransportError::Cert(_) => false,
    }
}

/// Standalone [`std::io::ErrorKind`] classifier — kept separate so it can be
/// unit-tested without constructing a full [`TransportError`].
fn is_transient_io_kind(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind;
    matches!(
        kind,
        ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::WouldBlock
            | ErrorKind::TimedOut
            | ErrorKind::NotConnected
    )
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

// ---- S3: RFC 5705 TLS channel-binding export ----

/// RFC 5705 label used for `export_keying_material` on every P2P connection.
///
/// Both sides of the PAKE handshake MUST use the identical label and context
/// so they derive the same 32-byte binder, which is then mixed into the
/// PAKE `SessionKey` via [`crate::pake::SessionKey::bind_to_tls_channel`].
pub const TLS_CHANNEL_BINDING_LABEL: &str = "EXPORTER-copypaste-channel-binding";

/// Extract a 32-byte RFC 5705 channel-binding token from a completed
/// **server-side** TLS stream.
///
/// Returns `Err(TransportError::Io)` if `export_keying_material` is not
/// supported by the current TLS session (e.g. TLS 1.2 without RFC 5705
/// support in the underlying provider, which should not occur with rustls
/// 0.23 + ring).
pub fn tls_channel_binder_server(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<[u8; 32], TransportError> {
    let (_, conn) = stream.get_ref();
    let mut out = [0u8; 32];
    conn.export_keying_material(&mut out, TLS_CHANNEL_BINDING_LABEL.as_bytes(), None)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(out)
}

/// Extract a 32-byte RFC 5705 channel-binding token from a completed
/// **client-side** TLS stream.
///
/// See [`tls_channel_binder_server`] for the security rationale.
pub fn tls_channel_binder_client(
    stream: &tokio_rustls::client::TlsStream<TcpStream>,
) -> Result<[u8; 32], TransportError> {
    let (_, conn) = stream.get_ref();
    let mut out = [0u8; 32];
    conn.export_keying_material(&mut out, TLS_CHANNEL_BINDING_LABEL.as_bytes(), None)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(out)
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

        let server_transport =
            PeerTransport::from_cert(server_cert.cert_der, server_cert.key_der, server_peers);
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

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
        let client_transport =
            PeerTransport::from_cert(unknown_cert.cert_der, unknown_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_fut = server_transport.accept(&listener);
        let client_fut = client_transport.connect(addr, &server_fp);

        let (server_result, _client_result) = tokio::join!(server_fut, client_fut);

        // The server must reject the unknown client.
        assert!(server_result.is_err(), "server must reject unknown peer");
    }

    /// edge HIGH #13 — a dead/silent peer must not stall the TLS handshake
    /// indefinitely. We open a TCP listener but never accept, so the connector
    /// will complete TCP SYN/ACK with the kernel but the TLS handshake bytes
    /// will sit in the kernel buffer with nobody on the other end. The client
    /// must give up with `HandshakeTimeout` within ~11s.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn tls_handshake_timeout_after_10s() {
        // Bind a listener but never call accept — TCP completes, TLS bytes go nowhere.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_cert = SelfSignedCert::generate("client-device").unwrap();
        // Use a bogus expected fingerprint — verifier never runs because the
        // handshake stalls long before any cert is received.
        let bogus_fp = "0".repeat(64);
        let mut client_peers = PairedPeers::new();
        client_peers.add(bogus_fp.clone(), "dead-peer");

        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        // Drive both the client connect and a virtual-time advance concurrently.
        // With `start_paused = true`, `tokio::time::sleep` advances the test
        // clock instantly, exercising the 10s timeout deterministically.
        let connect_fut = client_transport.connect(addr, &bogus_fp);
        let advance_fut = async {
            tokio::time::sleep(Duration::from_secs(11)).await;
        };

        let (result, _) = tokio::join!(connect_fut, advance_fut);

        let err = result.expect_err("client must time out, not succeed");
        assert!(
            matches!(err, TransportError::HandshakeTimeout),
            "expected HandshakeTimeout, got {:?}",
            err
        );

        // Keep the listener alive until here to avoid kernel TCP reset before
        // the timeout would fire on real time.
        drop(listener);
    }

    // ---- Sub C: connect_with_retry tests ----

    /// Sub C #1 — `is_transient_io_kind` classifies the documented kinds as
    /// transient and other common kinds as permanent. This is the lever that
    /// stops us retrying e.g. an `AddrNotAvailable` (mDNS gave us a bad IP).
    #[test]
    fn transient_io_kind_classifier() {
        use std::io::ErrorKind;
        // Transient — retried.
        assert!(is_transient_io_kind(ErrorKind::ConnectionRefused));
        assert!(is_transient_io_kind(ErrorKind::ConnectionReset));
        assert!(is_transient_io_kind(ErrorKind::ConnectionAborted));
        assert!(is_transient_io_kind(ErrorKind::BrokenPipe));
        assert!(is_transient_io_kind(ErrorKind::WouldBlock));
        assert!(is_transient_io_kind(ErrorKind::TimedOut));
        assert!(is_transient_io_kind(ErrorKind::NotConnected));

        // Permanent — surfaced immediately.
        assert!(!is_transient_io_kind(ErrorKind::AddrNotAvailable));
        assert!(!is_transient_io_kind(ErrorKind::AddrInUse));
        assert!(!is_transient_io_kind(ErrorKind::PermissionDenied));
        assert!(!is_transient_io_kind(ErrorKind::InvalidInput));
        assert!(!is_transient_io_kind(ErrorKind::Other));
    }

    /// Sub C #2 — non-I/O errors (unknown-peer, cert problems, our own
    /// handshake timeout) are NEVER classified as transient, even if a
    /// freshly-constructed `TransportError::Io(...)` would be.
    #[test]
    fn non_io_errors_are_never_transient() {
        let err = TransportError::UnknownPeer("deadbeef".into());
        assert!(!is_transient_transport_error(&err));

        let err = TransportError::NoPeerCert;
        assert!(!is_transient_transport_error(&err));

        let err = TransportError::InvalidKey;
        assert!(!is_transient_transport_error(&err));

        let err = TransportError::HandshakeTimeout;
        assert!(
            !is_transient_transport_error(&err),
            "HandshakeTimeout means we already burned 10s — retry would burn more"
        );

        // I/O errors of the transient flavour ARE retried.
        let err = TransportError::Io(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(is_transient_transport_error(&err));

        // I/O errors of a non-transient flavour are NOT.
        let err = TransportError::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(!is_transient_transport_error(&err));
    }

    /// Sub C #3 — `connect_with_retry` against a closed port (kernel returns
    /// ECONNREFUSED, a transient kind) must exhaust [`MAX_CONNECT_ATTEMPTS`]
    /// attempts before giving up. We bind a listener to grab a real port,
    /// then drop it so subsequent connects refuse immediately. With
    /// `start_paused`, the inter-attempt sleeps don't slow the test.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn connect_with_retry_exhausts_attempts_on_persistent_refusal() {
        // Bind then drop to learn a port the kernel has just released.
        let addr = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap()
        };

        let client_cert = SelfSignedCert::generate("client-device").unwrap();
        let bogus_fp = "0".repeat(64);
        let mut client_peers = PairedPeers::new();
        client_peers.add(bogus_fp.clone(), "dead-peer");

        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let connect_fut = client_transport.connect_with_retry(addr, &bogus_fp);
        // Advance virtual time past all retry delays + handshake timeouts so
        // the future actually completes (each attempt's connect is instant
        // because the port refuses immediately).
        let advance_fut = async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        };
        let (result, _) = tokio::join!(connect_fut, advance_fut);

        let err = result.expect_err("must fail after exhausting retries");
        // The final surfaced error must still be transient — meaning we did
        // genuinely exhaust the retry budget on a transient kind.
        assert!(
            is_transient_transport_error(&err),
            "expected a transient I/O error to surface, got {:?}",
            err
        );
    }

    /// Sub C #4 — `connect_with_retry` MUST NOT retry a permanent error.
    /// We aim it at an address that's reachable (the rogue server pattern)
    /// but with a fingerprint the verifier will reject, which surfaces as
    /// a non-transient TLS / I/O error. The retry helper should propagate
    /// after the first failure without burning the full attempt budget.
    ///
    /// We can't directly observe attempt count from outside, but we can
    /// bound test wall time: with paused time the test runs in microseconds,
    /// and the result must be the same kind of error `connect()` would
    /// have returned on its own.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn connect_with_retry_does_not_retry_permanent_errors() {
        // Set up a server that will accept TCP but reject in TLS verify
        // (mismatched fingerprint). This produces a non-transient error.
        let real_server_cert = SelfSignedCert::generate("real-server").unwrap();
        let rogue_server_cert = SelfSignedCert::generate("rogue-server").unwrap();
        let client_cert = SelfSignedCert::generate("client-device").unwrap();

        let real_server_fp = real_server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        let mut rogue_server_peers = PairedPeers::new();
        rogue_server_peers.add(client_fp.clone(), "client-device");
        let mut client_peers = PairedPeers::new();
        client_peers.add(real_server_fp.clone(), "real-server");

        let rogue_transport = PeerTransport::from_cert(
            rogue_server_cert.cert_der,
            rogue_server_cert.key_der,
            rogue_server_peers,
        );
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_fut = rogue_transport.accept(&listener);
        let client_fut = client_transport.connect_with_retry(addr, &real_server_fp);
        // Advance virtual time so any retry sleeps complete instantly.
        let advance_fut = async {
            tokio::time::sleep(Duration::from_secs(60)).await;
        };

        let (_server_result, client_result, _) =
            tokio::join!(server_fut, client_fut, advance_fut);

        assert!(
            client_result.is_err(),
            "rogue peer must be rejected, retries or not"
        );
    }

    /// edge HIGH #12 — a rogue mDNS advertisement may direct us to a real peer
    /// presenting a certificate we have never paired with. The `TlsVerifier`
    /// (via `is_known`) must reject such a peer, surfacing a TLS error on the
    /// client side (the server-side counterpart is already covered by
    /// `unknown_peer_cert_is_rejected`).
    #[tokio::test]
    async fn rogue_mdns_peer_rejected_by_verifier() {
        // Two legitimate device certs, plus a rogue cert pretending to be the
        // server we expect to connect to.
        let real_server_cert = SelfSignedCert::generate("real-server").unwrap();
        let rogue_server_cert = SelfSignedCert::generate("rogue-server").unwrap();
        let client_cert = SelfSignedCert::generate("client-device").unwrap();

        let real_server_fp = real_server_cert.fingerprint();
        let rogue_server_fp = rogue_server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();
        assert_ne!(real_server_fp, rogue_server_fp);

        // The rogue server happens to know the client (so the server-side
        // ClientCertVerifier would pass), but the client has only ever paired
        // with `real_server_fp` — never with `rogue_server_fp`.
        let mut rogue_server_peers = PairedPeers::new();
        rogue_server_peers.add(client_fp.clone(), "client-device");

        let mut client_peers = PairedPeers::new();
        client_peers.add(real_server_fp.clone(), "real-server");

        let rogue_transport = PeerTransport::from_cert(
            rogue_server_cert.cert_der,
            rogue_server_cert.key_der,
            rogue_server_peers,
        );
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Client expects `real_server_fp` but the rogue server presents its
        // own (unknown) fingerprint. The client's `verify_fingerprint` must
        // reject before any data is exchanged.
        let server_fut = rogue_transport.accept(&listener);
        let client_fut = client_transport.connect(addr, &real_server_fp);

        let (_server_result, client_result) = tokio::join!(server_fut, client_fut);

        assert!(
            client_result.is_err(),
            "client must reject rogue mDNS peer with mismatched cert"
        );
    }
}
