//! Unauthenticated bootstrap TLS channel for PAKE pairing (P2P Phase 1).
//!
//! # Why a separate, unauthenticated channel?
//!
//! The production [`crate::transport::PeerTransport`] requires *mutual*
//! certificate-fingerprint pinning: both ends must already know the other's
//! fingerprint (it lives in [`crate::transport::PairedPeers`]). That is a
//! chicken-and-egg problem for *first* pairing — neither side knows the other
//! yet.
//!
//! The bootstrap channel breaks the cycle. It is a TCP+TLS channel where
//! **both sides accept any certificate** (no pinning). Authentication is
//! provided out-of-band by the PAKE handshake: both ends derive the same
//! 32-byte [`crate::pake::SessionKey`] only if they share the QR pairing
//! secret, so a man-in-the-middle who cannot read the QR cannot complete the
//! handshake. The cert fingerprints are exchanged over this same channel and
//! their authenticity follows from PAKE success.
//!
//! TLS is still used (rather than plain TCP) so the PAKE messages and the
//! exchanged fingerprints are encrypted in transit on the LAN, and so the same
//! self-signed device certificate is presented that the *subsequent* pinned
//! mTLS sessions will use — letting each side learn the cert fingerprint it
//! must pin later.
//!
//! Channel binding (S3): after PAKE completes, each side mixes the RFC 5705 TLS
//! exporter for *this* TLS session into the PAKE key
//! ([`SessionKey::bind_to_tls_channel`]) and the two ends exchange
//! role-separated confirmation tags ([`crate::pake::channel_confirmation_tag`]),
//! compared in constant time. This binds pairing authenticity to the specific
//! bootstrap TLS session: a relay/MitM that bridges PAKE over two separate TLS
//! connections derives a different binder per leg, so the tags never match and
//! pairing is aborted.
//!
//! # Wire protocol (over the framed TLS stream)
//!
//! Length-delimited frames (same codec as [`crate::transport`]):
//!
//! ```text
//! Initiator (client)                         Responder (server)
//!   | --- 1. PAKE message1            -->  |
//!   | --- 2. own cert fingerprint     -->  |
//!   | --- 3. own P2P sync addr        -->  |
//!   | <-- 4. PAKE message2             --- |
//!   | <-- 5. own cert fingerprint      --- |
//!   | <-- 6. own P2P sync addr         --- |
//!   | --- 7. PAKE message3            -->  |
//!   | == both sides hold the same SessionKey == |
//!   | <-- 8. responder confirm tag     --- |
//!   | --- 9. initiator confirm tag    -->  |
//!   | == both confirm tags verified (constant-time) == |
//! ```
//!
//! On success each side returns the *peer's* cert fingerprint, the peer's P2P
//! sync-listener address, and the derived [`crate::pake::SessionKey`]. The peer
//! fingerprint sent in the frame is cross-checked against the fingerprint of the
//! certificate actually presented during the TLS handshake, so a peer cannot
//! advertise one fingerprint in the frame while presenting a different
//! certificate.
//!
//! ## Wire protocol version
//!
//! The sync-address frames (3 and 6) were added in P2P Phase 2. Both ends are
//! shipped together (there is no mixed-version pairing across hosts at this
//! stage), so the frame order is fixed rather than negotiated; the address frame
//! immediately follows each side's fingerprint frame. If cross-version pairing
//! ever becomes a requirement, prepend a version byte here.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error as TlsError, ServerConfig,
    SignatureScheme,
};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::cert::fingerprint_of;
use crate::pake::{
    channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile, SessionKey,
    CONFIRM_TAG_LEN,
};
use crate::transport::{
    tls_channel_binder_client, tls_channel_binder_server, DeviceFingerprint, TransportError,
    TLS_HANDSHAKE_TIMEOUT,
};

/// Maximum time the responder bootstrap listener waits for the single inbound
/// pairing connection before giving up. Kept generous: the pairing window is
/// driven by the QR TTL, and the user has to scan/confirm in between.
pub const BOOTSTRAP_ACCEPT_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound on a single PAKE/fingerprint frame. PAKE messages are a few
/// hundred bytes and fingerprints are 64 hex chars; 64 KiB is a wide margin
/// that still rejects a desynced peer flooding a huge length prefix.
const MAX_FRAME_BYTES: usize = 64 * 1024;

/// rustls verifier that accepts **any** peer certificate without pinning.
///
/// Used only on the bootstrap channel. It still requires the peer to *present*
/// a certificate (so we can learn its fingerprint and so the TLS handshake
/// completes with client auth on the server side), but performs no
/// chain/expiry/hostname/fingerprint validation. Authentication is the PAKE
/// handshake's job, not TLS's, on this channel.
#[derive(Debug)]
struct AcceptAnyCert;

impl AcceptAnyCert {
    /// Signature schemes we accept — delegate to the ring provider's full set.
    fn schemes() -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // Intentionally accept any server cert — PAKE authenticates the peer.
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::schemes()
    }
}

impl ClientCertVerifier for AcceptAnyCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        // Intentionally accept any client cert — PAKE authenticates the peer.
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        Self::schemes()
    }
}

/// Outcome of a completed bootstrap PAKE exchange.
pub struct BootstrapPairing {
    /// The peer's certificate fingerprint (hex SHA-256 of its cert DER), as
    /// observed on the TLS handshake AND confirmed to match the value the peer
    /// sent in-band. This is the value the caller pins in `PairedPeers`.
    pub peer_fingerprint: DeviceFingerprint,
    /// The peer's P2P sync-listener address (`host:port`), sent in-band during
    /// the exchange. The caller persists this to `peers.json` so the Phase 3
    /// outbound connector can dial the peer directly. May be empty if the peer
    /// did not advertise an address.
    pub peer_sync_addr: String,
    /// The 32-byte PAKE session key (identical on both sides on success).
    pub session_key: SessionKey,
}

/// A bootstrap TLS responder listener bound to an ephemeral port.
///
/// Construct with [`BootstrapResponder::bind`], read [`BootstrapResponder::addr`]
/// into the QR `addr_hint`, then call [`BootstrapResponder::run`] to accept one
/// connection and drive the responder side of the PAKE handshake over it.
pub struct BootstrapResponder {
    listener: TcpListener,
    acceptor: TlsAcceptor,
    own_cert_der: Vec<u8>,
    own_fingerprint: DeviceFingerprint,
}

impl BootstrapResponder {
    /// Bind an ephemeral bootstrap listener on `0.0.0.0:0` and TLS-wrap it with
    /// the daemon's self-signed certificate (the same cert whose fingerprint the
    /// pairing QR advertises).
    ///
    /// # Errors
    /// Returns [`TransportError::Io`] if the bind fails or
    /// [`TransportError::TlsConfig`] if the TLS config cannot be built.
    pub async fn bind(cert_der: Vec<u8>, key_der: Vec<u8>) -> Result<Self, TransportError> {
        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let own_fingerprint = fingerprint_of(&cert_der);

        let cert = CertificateDer::from(cert_der.clone());
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
        let private_key = PrivateKeyDer::Pkcs8(key);

        // Require the client to present a cert (so we learn its fingerprint),
        // but accept any cert — PAKE is the real authenticator on this channel.
        let server_config = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(AcceptAnyCert))
            .with_single_cert(vec![cert], private_key)
            .map_err(TransportError::TlsConfig)?;

        Ok(Self {
            listener,
            acceptor: TlsAcceptor::from(Arc::new(server_config)),
            own_cert_der: cert_der,
            own_fingerprint,
        })
    }

    /// The bound local address (`host:port`) to advertise in the QR `addr_hint`.
    ///
    /// The listener binds `0.0.0.0:0`; this returns the loopback-usable port via
    /// the OS-assigned ephemeral port.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Our own cert fingerprint (sent to the initiator over the channel).
    pub fn fingerprint(&self) -> &str {
        &self.own_fingerprint
    }

    /// Accept ONE inbound bootstrap connection (within
    /// [`BOOTSTRAP_ACCEPT_TIMEOUT`]) and run the responder side of the PAKE
    /// handshake over the TLS stream.
    ///
    /// `password` is the PAKE password derived from the QR token. A fresh
    /// [`PasswordFile`] is registered from it for this single handshake.
    /// `sync_addr` is this device's own P2P sync-listener `host:port`, sent
    /// in-band so the initiator can persist it for the Phase 3 connector.
    ///
    /// # Errors
    /// * [`TransportError::HandshakeTimeout`] if no connection / TLS handshake
    ///   completes in time.
    /// * [`TransportError::Io`] for socket / framing errors or a PAKE failure
    ///   (surfaced as `io::Error::other`), or a fingerprint mismatch between the
    ///   TLS cert and the in-band frame.
    pub async fn run(
        self,
        password: &str,
        sync_addr: &str,
    ) -> Result<BootstrapPairing, TransportError> {
        // Accept exactly one inbound TCP connection within the window.
        let (tcp_stream, peer_addr) =
            match tokio::time::timeout(BOOTSTRAP_ACCEPT_TIMEOUT, self.listener.accept()).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout = ?BOOTSTRAP_ACCEPT_TIMEOUT,
                        "bootstrap responder timed out waiting for inbound pairing connection"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };
        tracing::debug!(peer_addr = %peer_addr, "bootstrap: inbound TCP connection");

        let tls_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, self.acceptor.accept(tcp_stream))
                .await
            {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!("bootstrap: TLS server handshake timed out");
                    return Err(TransportError::HandshakeTimeout);
                }
            };

        // The cert fingerprint the peer actually presented in TLS.
        let tls_peer_fp = {
            let (_, conn) = tls_stream.get_ref();
            let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
            let first = certs.first().ok_or(TransportError::NoPeerCert)?;
            fingerprint_of(first.as_ref())
        };

        // RFC 5705 channel binder for THIS TLS session (extracted before the
        // stream is moved into `Framed`). Mixed into the PAKE key below so the
        // pairing is bound to this exact TLS channel.
        let tls_binder = tls_channel_binder_server(&tls_stream)?;

        let mut framed = Framed::new(tls_stream, length_codec());

        // PasswordFile for this single handshake, derived from the QR password.
        let password_file = PasswordFile::register(password)
            .map_err(|e| io_other(format!("PasswordFile::register: {e}")))?;

        // Frame 1 ← initiator's PAKE message1.
        let msg1 = recv_frame(&mut framed).await?;
        // Frame 2 ← initiator's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;
        // Frame 3 ← initiator's P2P sync-listener address (Phase 2).
        let peer_sync_addr = recv_sync_addr(&mut framed).await?;

        // The fingerprint the peer claims in-band MUST match the cert it
        // presented in the TLS handshake — otherwise a peer could pin us to a
        // cert it does not actually hold.
        if frame_peer_fp != tls_peer_fp {
            return Err(io_other(format!(
                "bootstrap: initiator frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
            )));
        }

        let (responder, msg2) = PakeResponder::respond(&password_file, &msg1)
            .map_err(|e| io_other(format!("PAKE respond: {e}")))?;

        // Frame 4 → our PAKE message2.
        send_frame(&mut framed, &msg2).await?;
        // Frame 5 → our cert fingerprint.
        send_frame(&mut framed, self.own_fingerprint.as_bytes()).await?;
        // Frame 6 → our P2P sync-listener address (Phase 2).
        send_frame(&mut framed, sync_addr.as_bytes()).await?;

        // Frame 7 ← initiator's PAKE finalisation.
        let msg3 = recv_frame(&mut framed).await?;
        let session_key = responder
            .finish(&msg3)
            .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

        // Channel-binding confirmation (S3). Bind the PAKE key to this TLS
        // session, then exchange role-separated confirmation tags. A match in
        // constant time proves the peer shares the same PAKE key AND the same
        // TLS channel — a relay bridging two TLS sessions would derive a
        // different binder per leg, so its tags would never match.
        let bound_key = session_key.bind_to_tls_channel(&tls_binder);
        let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
        let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);

        // Frame 8 → our confirmation tag.
        send_frame(&mut framed, &own_tag).await?;
        // Frame 9 ← initiator's confirmation tag.
        let peer_tag = recv_confirmation_tag(&mut framed).await?;
        if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
            return Err(io_other(
                "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
            ));
        }

        // Touch own_cert_der so the field is not flagged unused; the bytes are
        // already consumed via the TLS config but the DER is kept for any future
        // re-bind without regenerating.
        debug_assert!(!self.own_cert_der.is_empty());

        Ok(BootstrapPairing {
            peer_fingerprint: tls_peer_fp,
            peer_sync_addr,
            session_key,
        })
    }
}

/// Dial a bootstrap responder at `addr` over TLS **without** cert pinning and
/// run the initiator side of the PAKE handshake.
///
/// `cert_der` / `key_der` are this device's self-signed cert and key (presented
/// to the responder so it learns our fingerprint). `password` is the PAKE
/// password derived from the QR token. `sync_addr` is this device's own P2P
/// sync-listener `host:port`, sent in-band so the responder can persist it for
/// the Phase 3 connector.
///
/// # Errors
/// Mirrors [`BootstrapResponder::run`] — TLS / socket / framing errors and PAKE
/// failures (including a wrong password, surfaced from `client.finish`).
pub async fn run_initiator(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    password: &str,
    sync_addr: &str,
) -> Result<BootstrapPairing, TransportError> {
    let own_fingerprint = fingerprint_of(&cert_der);

    let cert = CertificateDer::from(cert_der);
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(key_der);
    let private_key = PrivateKeyDer::Pkcs8(key);

    let client_config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_client_auth_cert(vec![cert], private_key)
        .map_err(TransportError::TlsConfig)?;
    let connector = TlsConnector::from(Arc::new(client_config));

    let tcp_stream =
        match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(res) => res?,
            Err(_elapsed) => {
                tracing::warn!(peer_addr = %addr, "bootstrap: TCP connect timed out");
                return Err(TransportError::HandshakeTimeout);
            }
        };

    // rustls requires a ServerName; identity is verified by PAKE, not SNI, so a
    // fixed placeholder is fine (and is what the pinned transport uses too).
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
            tracing::warn!(peer_addr = %addr, "bootstrap: TLS client handshake timed out");
            return Err(TransportError::HandshakeTimeout);
        }
    };

    // The cert fingerprint the responder actually presented in TLS.
    let tls_peer_fp = {
        let (_, conn) = tls_stream.get_ref();
        let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
        let first = certs.first().ok_or(TransportError::NoPeerCert)?;
        fingerprint_of(first.as_ref())
    };

    // RFC 5705 channel binder for THIS TLS session (extracted before the stream
    // is moved into `Framed`). Mixed into the PAKE key below.
    let tls_binder = tls_channel_binder_client(&tls_stream)?;

    let mut framed = Framed::new(tls_stream, length_codec());

    let (client, msg1) =
        PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

    // Frame 1 → our PAKE message1.
    send_frame(&mut framed, &msg1).await?;
    // Frame 2 → our cert fingerprint.
    send_frame(&mut framed, own_fingerprint.as_bytes()).await?;
    // Frame 3 → our P2P sync-listener address (Phase 2).
    send_frame(&mut framed, sync_addr.as_bytes()).await?;

    // Frame 4 ← responder's PAKE message2.
    let msg2 = recv_frame(&mut framed).await?;
    // Frame 5 ← responder's cert fingerprint.
    let frame_peer_fp = recv_fingerprint(&mut framed).await?;
    // Frame 6 ← responder's P2P sync-listener address (Phase 2).
    let peer_sync_addr = recv_sync_addr(&mut framed).await?;

    if frame_peer_fp != tls_peer_fp {
        return Err(io_other(format!(
            "bootstrap: responder frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
        )));
    }

    let (session_key, msg3) = client
        .finish(&msg2)
        .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

    // Frame 7 → our PAKE finalisation.
    send_frame(&mut framed, &msg3).await?;

    // Channel-binding confirmation (S3). See `BootstrapResponder::run` for the
    // rationale — bind to this TLS session and exchange role-separated tags,
    // aborting on any mismatch (relay MitM defence).
    let bound_key = session_key.bind_to_tls_channel(&tls_binder);
    let own_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
    let expected_peer_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);

    // Frame 8 ← responder's confirmation tag.
    let peer_tag = recv_confirmation_tag(&mut framed).await?;
    // Frame 9 → our confirmation tag.
    send_frame(&mut framed, &own_tag).await?;
    if peer_tag.ct_eq(&expected_peer_tag).unwrap_u8() != 1 {
        return Err(io_other(
            "bootstrap: channel-binding confirmation mismatch — possible relay MitM, pairing aborted".into(),
        ));
    }

    Ok(BootstrapPairing {
        peer_fingerprint: tls_peer_fp,
        peer_sync_addr,
        session_key,
    })
}

// ── framing helpers ───────────────────────────────────────────────────────────

fn length_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_FRAME_BYTES)
        .new_codec()
}

fn io_other(msg: String) -> TransportError {
    TransportError::Io(std::io::Error::other(msg))
}

async fn send_frame<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    body: &[u8],
) -> Result<(), TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    framed
        .send(bytes::Bytes::copy_from_slice(body))
        .await
        .map_err(TransportError::Io)
}

async fn recv_frame<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<Vec<u8>, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    match framed.next().await {
        Some(Ok(bytes)) => Ok(bytes.to_vec()),
        Some(Err(e)) => Err(TransportError::Io(e)),
        None => Err(io_other(
            "bootstrap: peer closed before sending frame".into(),
        )),
    }
}

/// Receive a peer's channel-binding confirmation tag frame (S3).
///
/// Enforces the exact [`CONFIRM_TAG_LEN`] so a desynced or malicious peer
/// cannot smuggle a short/long frame into the constant-time compare slot.
async fn recv_confirmation_tag<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<[u8; CONFIRM_TAG_LEN], TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    let tag: [u8; CONFIRM_TAG_LEN] = bytes.as_slice().try_into().map_err(|_| {
        io_other(format!(
            "bootstrap: confirmation tag wrong length ({} bytes)",
            bytes.len()
        ))
    })?;
    Ok(tag)
}

async fn recv_fingerprint<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<String, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    let fp = String::from_utf8(bytes)
        .map_err(|e| io_other(format!("bootstrap: fingerprint not UTF-8: {e}")))?;
    // A real cert fingerprint is 64 lowercase hex chars.
    if fp.len() != 64 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io_other(format!(
            "bootstrap: malformed peer fingerprint ({} bytes)",
            fp.len()
        )));
    }
    Ok(fp)
}

/// Upper bound on a peer's advertised sync-listener address. A `host:port` is at
/// most a few dozen bytes (IPv6 + port ≈ 47); 256 is a generous ceiling that
/// still rejects a desynced peer sending a huge frame in this slot.
const MAX_SYNC_ADDR_BYTES: usize = 256;

/// Receive a peer's P2P sync-listener `host:port` address frame (Phase 2).
///
/// The address is opaque to the bootstrap layer (it is parsed/validated by the
/// daemon when it dials in Phase 3); this only enforces UTF-8 and a sane length
/// bound so a malformed frame cannot smuggle arbitrary bytes into `peers.json`.
/// An empty frame is accepted and returned as an empty string (the peer simply
/// did not advertise an address).
async fn recv_sync_addr<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<String, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    if bytes.len() > MAX_SYNC_ADDR_BYTES {
        return Err(io_other(format!(
            "bootstrap: peer sync address too long ({} bytes)",
            bytes.len()
        )));
    }
    String::from_utf8(bytes)
        .map_err(|e| io_other(format!("bootstrap: sync address not UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::SelfSignedCert;

    /// Two endpoints over a real loopback TCP/TLS socket complete PAKE, the S3
    /// channel-binding confirmation exchange, and converge on the same session
    /// key, learning each other's fingerprints. Both `run`/`run_initiator`
    /// returning `Ok` proves the confirmation tags matched.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bootstrap_pake_over_tls_loopback_succeeds() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

        let responder_fp = responder_cert.fingerprint();
        let initiator_fp = initiator_cert.fingerprint();
        assert_ne!(responder_fp, initiator_fp);

        let password = "shared-qr-secret-123456";

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();
        let resp_fp_expected = responder.fingerprint().to_string();
        assert_eq!(resp_fp_expected, responder_fp);

        let pw = password.to_string();
        let resp_sync_addr = "127.0.0.1:7001";
        let responder_task = tokio::spawn(async move { responder.run(&pw, resp_sync_addr).await });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_pw = password.to_string();
        let init_sync_addr = "127.0.0.1:7002";
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
                init_sync_addr,
            )
            .await
        });

        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let resp = resp_res.expect("responder join").expect("responder pake");
        let init = init_res.expect("initiator join").expect("initiator pake");

        // Session keys converge — the PAKE security goal, over a real network stack.
        assert_eq!(
            resp.session_key.as_bytes(),
            init.session_key.as_bytes(),
            "both endpoints must derive the same PAKE session key over TLS"
        );

        // Each side learned the other's real cert fingerprint.
        assert_eq!(resp.peer_fingerprint, initiator_fp);
        assert_eq!(init.peer_fingerprint, responder_fp);

        // Phase 2: each side also learned the other's P2P sync-listener address.
        assert_eq!(resp.peer_sync_addr, init_sync_addr);
        assert_eq!(init.peer_sync_addr, resp_sync_addr);
    }

    /// Wrong password: the initiator's PAKE finish must fail, and the responder
    /// must not produce a session key.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bootstrap_pake_wrong_password_fails() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        let responder_task =
            tokio::spawn(
                async move { responder.run("the-right-password", "127.0.0.1:7003").await },
            );

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "the-WRONG-password",
                "127.0.0.1:7004",
            )
            .await
        });

        let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
        let init = init_res.expect("initiator join");
        assert!(init.is_err(), "initiator must fail on wrong password");
        let resp = resp_res.expect("responder join");
        assert!(
            resp.is_err(),
            "responder must not derive a key on wrong password"
        );
    }

    /// Relay MitM: an attacker who knows the correct PAKE password but cannot
    /// keep a single TLS channel end-to-end. The relay terminates TLS toward the
    /// initiator and opens a *separate* TLS session to the real responder, then
    /// blindly pumps the opaque PAKE/confirmation frames between the two legs.
    ///
    /// PAKE itself still completes (the bytes are forwarded verbatim), but the
    /// RFC 5705 channel binder differs on each TLS leg, so the channel-bound
    /// confirmation tags do not match and BOTH endpoints must reject pairing.
    /// This is the exact attack S3 channel binding defends against.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn bootstrap_relay_mitm_is_rejected_by_channel_binding() {
        use tokio::io::{copy, AsyncWriteExt};

        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
        let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
        let relay_cert = SelfSignedCert::generate("relay-mitm-device").unwrap();

        let password = "shared-qr-secret-relay";

        // Real responder.
        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let responder_port = responder.local_addr().expect("local addr").port();
        let pw = password.to_string();
        let responder_task =
            tokio::spawn(async move { responder.run(&pw, "127.0.0.1:7005").await });

        // Relay listener: TLS server toward the initiator (accept any client cert).
        let relay_listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
        let relay_port = relay_listener.local_addr().unwrap().port();

        let relay_server_cfg = ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(AcceptAnyCert))
            .with_single_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay server cfg");
        let relay_acceptor = TlsAcceptor::from(Arc::new(relay_server_cfg));

        // Relay client config toward the real responder (accept any server cert,
        // present the relay's own cert).
        let relay_client_cfg = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
            .with_client_auth_cert(
                vec![CertificateDer::from(relay_cert.cert_der.clone())],
                PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                    relay_cert.key_der.clone(),
                )),
            )
            .expect("relay client cfg");
        let relay_connector = TlsConnector::from(Arc::new(relay_client_cfg));

        let relay_task = tokio::spawn(async move {
            let (inbound, _) = relay_listener.accept().await.expect("relay accept");
            let init_tls = relay_acceptor
                .accept(inbound)
                .await
                .expect("relay tls accept");

            let upstream = TcpStream::connect(("127.0.0.1", responder_port))
                .await
                .expect("relay->responder connect");
            let server_name = ServerName::try_from("copypaste.peer").unwrap();
            let resp_tls = relay_connector
                .connect(server_name, upstream)
                .await
                .expect("relay->responder tls");

            // Blindly pump bytes both directions between the two TLS legs.
            let (mut ir, mut iw) = tokio::io::split(init_tls);
            let (mut rr, mut rw) = tokio::io::split(resp_tls);
            let a = tokio::spawn(async move {
                let _ = copy(&mut ir, &mut rw).await;
                let _ = rw.shutdown().await;
            });
            let b = tokio::spawn(async move {
                let _ = copy(&mut rr, &mut iw).await;
                let _ = iw.shutdown().await;
            });
            let _ = tokio::join!(a, b);
        });

        // Initiator dials the RELAY (thinking it is the responder).
        let relay_addr: SocketAddr = ([127, 0, 0, 1], relay_port).into();
        let init_pw = password.to_string();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                relay_addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
                "127.0.0.1:7006",
            )
            .await
        });

        let (resp_res, init_res, _relay_res) =
            tokio::join!(responder_task, initiator_task, relay_task);

        let init = init_res.expect("initiator join");
        assert!(
            init.is_err(),
            "initiator must reject pairing — channel binding confirmation mismatch under relay MitM"
        );
        let resp = resp_res.expect("responder join");
        assert!(
            resp.is_err(),
            "responder must reject pairing — channel binding confirmation mismatch under relay MitM"
        );
    }
}
