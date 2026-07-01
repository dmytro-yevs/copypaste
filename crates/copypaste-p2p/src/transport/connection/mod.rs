//! Mutual-TLS connection layer: `PeerTransport` — the main entry point for
//! P2P TLS connections. Tuning constants live in `config`, the error type
//! and framed-stream aliases in `error`, and retry classification in
//! `retry`; all three are re-exported here so `connection::<Name>` still
//! resolves unchanged for `transport/mod.rs`'s `pub use`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::Framed;

use super::peers::{DeviceFingerprint, PairedPeers};
use crate::cert::{fingerprint_of, SelfSignedCert};
use crate::verifier::PeerCertVerifier;

mod config;
mod error;
mod retry;
#[cfg(test)]
mod tests;

pub use config::{
    CONNECT_RETRY_DELAY, MAX_CONNECT_ATTEMPTS, MAX_FRAME_BYTES, P2P_SNI_SENTINEL,
    TCP_CONNECT_TIMEOUT, TLS_HANDSHAKE_TIMEOUT,
};
pub use error::{PeerClientStream, PeerStream, TransportError};

use config::{enable_tcp_keepalive, length_codec};
// `is_transient_io_kind` is not called directly in this file — it is imported
// here (rather than only inside `retry.rs`) so `connection::tests`' `use
// super::*;` (unchanged from the pre-split single-file test module) still
// resolves `transient_io_kind_classifier`'s direct calls to it. Only the
// `tests` submodule (cfg(test)) actually references it, so gate the import
// itself the same way to avoid an unused-import warning in non-test builds.
#[cfg(test)]
use retry::is_transient_io_kind;
use retry::{is_transient_transport_error, peer_fingerprint_server};

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
    /// `ServerConfig`/`TlsAcceptor` built once and reused across all `accept()`
    /// calls. Constructed lazily on the first call so construction errors are
    /// still surfaced as `TransportError` rather than panics, while the hot
    /// path (steady-state accept loop) never rebuilds the config.
    cached_acceptor: std::sync::OnceLock<Arc<TlsAcceptor>>,
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
        let own_fingerprint = DeviceFingerprint(fingerprint_of(&cert_der));
        Self {
            own_cert_der: cert_der,
            own_key_der: key_der,
            own_fingerprint,
            peers: Arc::new(peers),
            cached_acceptor: std::sync::OnceLock::new(),
        }
    }

    /// Returns our device's certificate fingerprint.
    pub fn fingerprint(&self) -> &str {
        &self.own_fingerprint
    }

    /// Bind a TCP listener on `addr` and wait for one incoming mutual-TLS connection.
    ///
    /// On success, returns the remote `SocketAddr`, the verified peer
    /// certificate fingerprint, and a framed stream ready for message exchange.
    ///
    /// The fingerprint is the stable device identity (independent of the
    /// peer's ephemeral source port) — callers key per-peer connection state by
    /// it so a reconnect from a new port replaces, rather than duplicates, the
    /// previous connection (fix/p2p-c-review #4).
    pub async fn accept(
        &self,
        listener: &TcpListener,
    ) -> Result<(SocketAddr, DeviceFingerprint, PeerStream), TransportError> {
        // Build the TlsAcceptor exactly once across all accept() calls. The
        // ServerConfig embeds a PeerCertVerifier that holds an Arc<PairedPeers>,
        // so live peer-list updates (add/rotate/remove) are still reflected on
        // every handshake — only the TLS config scaffolding is cached, not the
        // peer set. Using OnceLock means no lock contention on the hot path.
        let acceptor = match self.cached_acceptor.get() {
            Some(a) => a.clone(),
            None => {
                // `OnceLock::get_or_try_init` is unstable, so build-then-`set`
                // on the stable API. A concurrent first-accept may win the
                // `set` race; either way we end up with a single cached value.
                let server_config = self.build_server_config()?;
                let built = Arc::new(TlsAcceptor::from(Arc::new(server_config)));
                let _ = self.cached_acceptor.set(built.clone());
                self.cached_acceptor.get().cloned().unwrap_or(built)
            }
        };

        let (tcp_stream, peer_addr) = listener.accept().await?;
        tracing::debug!(peer_addr = %peer_addr, "incoming TCP connection");
        enable_tcp_keepalive(&tcp_stream);

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

        let framed = Framed::new(tls_stream, length_codec());
        Ok((peer_addr, DeviceFingerprint(peer_fp), framed))
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
            match tokio::time::timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        peer_addr = %addr,
                        timeout = ?TCP_CONNECT_TIMEOUT,
                        "TCP connect timed out — classifying as transient for retry"
                    );
                    // Map to Io(TimedOut) so `is_transient_transport_error`
                    // classifies this as TRANSIENT and connect_with_retry can
                    // retry the mDNS-announce→listener race. HandshakeTimeout
                    // is reserved for the post-TCP TLS phase (permanent guard).
                    return Err(TransportError::Io(std::io::Error::from(
                        std::io::ErrorKind::TimedOut,
                    )));
                }
            };
        tracing::debug!(peer_addr = %addr, "TCP connection established");
        enable_tcp_keepalive(&tcp_stream);

        // rustls requires a ServerName even for mutual-TLS peer-to-peer.
        // We use a fixed placeholder since identity is verified by fingerprint.
        let server_name =
            ServerName::try_from(P2P_SNI_SENTINEL).expect("static server name is always valid");

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

        let framed = Framed::new(tls_stream, length_codec());
        Ok(framed)
    }

    /// Connect to a peer with bounded retries on transient I/O errors.
    ///
    /// Wraps [`Self::connect`] with up to [`MAX_CONNECT_ATTEMPTS`] attempts
    /// (one initial + N-1 retries), separated by [`CONNECT_RETRY_DELAY`].
    /// Only **transient** errors are retried — see `is_transient_transport_error`
    /// for the exhaustive list. Permanent errors (unknown-peer, TLS config,
    /// cert problems, handshake timeout) propagate on the first failure so
    /// callers don't waste time retrying a fundamentally broken setup.
    ///
    /// The intended use case is the brief race between mDNS announcement
    /// and the peer's TCP listener actually accepting connections, and
    /// transient LAN blips (cable bounce, brief Wi-Fi roaming). For
    /// long-haul relay reconnects with exponential backoff, see
    /// `copypaste_sync::backoff::BackoffScheduler`.
    pub async fn connect_with_retry(
        &self,
        addr: SocketAddr,
        expected_fingerprint: &str,
    ) -> Result<PeerClientStream, TransportError> {
        // Guard: a zero MAX_CONNECT_ATTEMPTS would leave last_err = None and
        // the final .expect() below would panic. Return a clear error instead.
        if MAX_CONNECT_ATTEMPTS == 0 {
            return Err(TransportError::Io(std::io::Error::other(
                "MAX_CONNECT_ATTEMPTS is 0; no connection attempt made",
            )));
        }
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
                        // ±50 ms jitter centred on the 100 ms base so
                        // concurrent peers that hit the same transient (e.g.
                        // mDNS race) don't lock-step their retries. Uses
                        // gen_range(0..100) − 50 to avoid modulo bias and to
                        // produce a symmetric window [base−50ms, base+50ms].
                        let raw_jitter: i64 = rand::thread_rng().gen_range(0..100);
                        let offset_ms = raw_jitter - 50; // centred: −50..+50
                        let delay = if offset_ms >= 0 {
                            CONNECT_RETRY_DELAY + Duration::from_millis(offset_ms as u64)
                        } else {
                            // Saturate at zero rather than underflow.
                            CONNECT_RETRY_DELAY
                                .saturating_sub(Duration::from_millis((-offset_ms) as u64))
                        };
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
