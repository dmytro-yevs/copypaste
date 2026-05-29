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
//! Channel binding (mixing the TLS exporter into the PAKE key) is intentionally
//! **not** done here — it is a later phase. See `SessionKey::bind_to_tls_channel`.
//!
//! # Wire protocol (over the framed TLS stream)
//!
//! Length-delimited frames (same codec as [`crate::transport`]):
//!
//! ```text
//! Initiator (client)                         Responder (server)
//!   | --- 1. PAKE message1            -->  |
//!   | --- 2. own cert fingerprint     -->  |
//!   | <-- 3. PAKE message2             --- |
//!   | <-- 4. own cert fingerprint      --- |
//!   | --- 5. PAKE message3            -->  |
//!   | == both sides hold the same SessionKey == |
//! ```
//!
//! On success each side returns the *peer's* cert fingerprint and the derived
//! [`crate::pake::SessionKey`]. The peer fingerprint sent in the frame is
//! cross-checked against the fingerprint of the certificate actually presented
//! during the TLS handshake, so a peer cannot advertise one fingerprint in the
//! frame while presenting a different certificate.

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
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::cert::fingerprint_of;
use crate::pake::{PakeInitiator, PakeResponder, PasswordFile, SessionKey};
use crate::transport::{DeviceFingerprint, TransportError, TLS_HANDSHAKE_TIMEOUT};

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
    ///
    /// # Errors
    /// * [`TransportError::HandshakeTimeout`] if no connection / TLS handshake
    ///   completes in time.
    /// * [`TransportError::Io`] for socket / framing errors or a PAKE failure
    ///   (surfaced as `io::Error::other`), or a fingerprint mismatch between the
    ///   TLS cert and the in-band frame.
    pub async fn run(self, password: &str) -> Result<BootstrapPairing, TransportError> {
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

        let mut framed = Framed::new(tls_stream, length_codec());

        // PasswordFile for this single handshake, derived from the QR password.
        let password_file = PasswordFile::register(password)
            .map_err(|e| io_other(format!("PasswordFile::register: {e}")))?;

        // Frame 1 ← initiator's PAKE message1.
        let msg1 = recv_frame(&mut framed).await?;
        // Frame 2 ← initiator's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;

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

        // Frame 3 → our PAKE message2.
        send_frame(&mut framed, &msg2).await?;
        // Frame 4 → our cert fingerprint.
        send_frame(&mut framed, self.own_fingerprint.as_bytes()).await?;

        // Frame 5 ← initiator's PAKE finalisation.
        let msg3 = recv_frame(&mut framed).await?;
        let session_key = responder
            .finish(&msg3)
            .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

        // Touch own_cert_der so the field is not flagged unused; the bytes are
        // already consumed via the TLS config but the DER is kept for any future
        // re-bind without regenerating.
        debug_assert!(!self.own_cert_der.is_empty());

        Ok(BootstrapPairing {
            peer_fingerprint: tls_peer_fp,
            session_key,
        })
    }
}

/// Dial a bootstrap responder at `addr` over TLS **without** cert pinning and
/// run the initiator side of the PAKE handshake.
///
/// `cert_der` / `key_der` are this device's self-signed cert and key (presented
/// to the responder so it learns our fingerprint). `password` is the PAKE
/// password derived from the QR token.
///
/// # Errors
/// Mirrors [`BootstrapResponder::run`] — TLS / socket / framing errors and PAKE
/// failures (including a wrong password, surfaced from `client.finish`).
pub async fn run_initiator(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    password: &str,
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

    let mut framed = Framed::new(tls_stream, length_codec());

    let (client, msg1) =
        PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

    // Frame 1 → our PAKE message1.
    send_frame(&mut framed, &msg1).await?;
    // Frame 2 → our cert fingerprint.
    send_frame(&mut framed, own_fingerprint.as_bytes()).await?;

    // Frame 3 ← responder's PAKE message2.
    let msg2 = recv_frame(&mut framed).await?;
    // Frame 4 ← responder's cert fingerprint.
    let frame_peer_fp = recv_fingerprint(&mut framed).await?;

    if frame_peer_fp != tls_peer_fp {
        return Err(io_other(format!(
            "bootstrap: responder frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
        )));
    }

    let (session_key, msg3) = client
        .finish(&msg2)
        .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

    // Frame 5 → our PAKE finalisation.
    send_frame(&mut framed, &msg3).await?;

    Ok(BootstrapPairing {
        peer_fingerprint: tls_peer_fp,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::SelfSignedCert;

    /// Two endpoints over a real loopback TCP/TLS socket complete PAKE and
    /// converge on the same session key, learning each other's fingerprints.
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
        let responder_task = tokio::spawn(async move { responder.run(&pw).await });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_pw = password.to_string();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
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

        let responder_task = tokio::spawn(async move { responder.run("the-right-password").await });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "the-WRONG-password",
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
}
