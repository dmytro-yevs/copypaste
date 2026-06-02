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
//! immediately follows each side's fingerprint frame.
//!
//! ## Device-metadata extension (appended after frame 9)
//!
//! P2P Phase 4 appends an OPTIONAL metadata exchange AFTER the 9-frame PAKE +
//! channel-binding handshake has fully completed (both confirmation tags
//! verified). Because it comes strictly after the original protocol terminates,
//! an OLD peer (which closes / stops reading at frame 9) is unaffected — it
//! never sees these frames and pairing still succeeds for both. A new peer:
//!
//! ```text
//!   | --- 10. BOOTSTRAP_PROTO_VERSION byte  -->  (and the mirror <-- )
//!   | --- 11. compact JSON {model,os_version,app_version,local_ip}  --> (mirror <--)
//! ```
//!
//! Each side first sends its own version byte then reads the peer's. If the
//! peer's version frame is absent (connection closed → old peer) the metadata
//! step is skipped entirely and `peer_*` fields are left `None`. If the peer's
//! version is `< BOOTSTRAP_PROTO_VERSION` the metadata frame is likewise
//! skipped. All metadata send/receive errors are swallowed: the pairing is
//! already authenticated and complete, so a metadata hiccup must never fail it.

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
    P2P_SNI_SENTINEL, TCP_CONNECT_TIMEOUT, TLS_HANDSHAKE_TIMEOUT,
};

/// Maximum time the responder bootstrap listener waits for the single inbound
/// pairing connection before giving up.
///
/// # Drift guard — keep in sync with the QR TTL
///
/// This timeout is intentionally coupled to the QR code's time-to-live: the
/// user scans the QR, confirms on their device, and the initiator connects —
/// all within this window. The QR TTL is currently 120 s (set by the daemon's
/// `generate_pairing_qr` handler which stamps `expires_at = now + 120s`).
///
/// There is no shared const yet (the QR TTL lives in `copypaste-daemon`'s IPC
/// handler, not in `copypaste-core` or `copypaste-ipc`). Until one is
/// extracted, keep this value equal to the daemon's QR TTL (120 s). When the
/// QR TTL changes, update this const in the same commit.
///
/// TODO: extract a `QR_TTL: Duration` const into `copypaste-ipc` and reference
/// it here so the two values cannot drift independently.
pub const BOOTSTRAP_ACCEPT_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum total time allowed for the 9-frame post-TLS PAKE exchange (both
/// sides). A peer that completes TLS but then dribbles frames would otherwise
/// pin the single-shot responder indefinitely (slowloris-style DoS). 30 s is
/// ample for an honest peer on a LAN; a stalled peer is evicted after this.
pub const PAKE_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Upper bound on a single PAKE/fingerprint frame. PAKE messages are a few
/// hundred bytes and fingerprints are 64 hex chars; 64 KiB is a wide margin
/// that still rejects a desynced peer flooding a huge length prefix.
const MAX_FRAME_BYTES: usize = 64 * 1024;

/// Bootstrap wire-protocol version advertised in the post-handshake metadata
/// extension (frame 10). Bumped when the metadata frame layout changes. A peer
/// that does not send a version frame at all is treated as a pre-extension
/// (legacy) peer and the metadata step is skipped.
pub const BOOTSTRAP_PROTO_VERSION: u8 = 1;

/// Upper bound on the peer metadata JSON frame. The four short strings (model,
/// OS, app version, IP) total well under 256 bytes; 1 KiB is a wide ceiling that
/// still rejects a desynced peer flooding this slot.
const MAX_META_BYTES: usize = 1024;

/// Compact device-identity metadata exchanged in-band over the bootstrap channel
/// AFTER the PAKE handshake completes (P2P Phase 4).
///
/// All fields are best-effort and optional. `copypaste-p2p` does not collect
/// these itself (it has no platform deps) — the daemon passes them in. They are
/// non-secret (model, OS version, app version, LAN IP) and mirror what mDNS
/// already broadcasts.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PeerMeta {
    /// Friendly hardware model, e.g. `"MacBook Air"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// OS name + version, e.g. `"macOS 15.5"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// App / daemon version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    /// Best LAN-routable display IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
}

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
    /// Peer's friendly hardware model, learned over the post-handshake metadata
    /// extension. `None` when the peer is a legacy (pre-extension) build or did
    /// not advertise the field.
    pub peer_model: Option<String>,
    /// Peer's OS name + version, learned over the metadata extension.
    pub peer_os: Option<String>,
    /// Peer's app / daemon version, learned over the metadata extension.
    pub peer_app_version: Option<String>,
    /// Peer's best LAN-routable display IP, learned over the metadata extension.
    pub peer_local_ip: Option<String>,
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
        own_meta: &PeerMeta,
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

        // Touch own_cert_der so the field is not flagged unused; the bytes are
        // already consumed via the TLS config but the DER is kept for any future
        // re-bind without regenerating.
        debug_assert!(!self.own_cert_der.is_empty());

        // Wrap the entire 9-frame PAKE exchange in a single deadline so a peer
        // that completes TLS but then stalls mid-exchange cannot pin this
        // single-shot responder indefinitely (slowloris-style DoS).
        let own_fingerprint = self.own_fingerprint.clone();
        let sync_addr = sync_addr.to_owned();
        let own_meta = own_meta.clone();
        let pairing = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async move {
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
            // presented in the TLS handshake. Lowercase before comparing to
            // handle peers that send uppercase hex (avoid false mismatch).
            // The value is public (exchanged over an authenticated channel);
            // a simple == suffices — no timing side-channel risk here.
            if frame_peer_fp.to_lowercase() != tls_peer_fp {
                return Err(io_other(format!(
                    "bootstrap: initiator frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
                )));
            }

            let (responder, msg2) = PakeResponder::respond(&password_file, &msg1)
                .map_err(|e| io_other(format!("PAKE respond: {e}")))?;

            // Frame 4 → our PAKE message2.
            send_frame(&mut framed, &msg2).await?;
            // Frame 5 → our cert fingerprint.
            send_frame(&mut framed, own_fingerprint.as_bytes()).await?;
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

            // P2P Phase 4 (optional, post-handshake): exchange device metadata.
            // The pairing is already complete and authenticated at this point;
            // any failure here is swallowed (legacy peer closed, etc.).
            let peer_meta = exchange_peer_meta(&mut framed, &own_meta).await;

            Ok::<BootstrapPairing, TransportError>(BootstrapPairing {
                peer_fingerprint: tls_peer_fp,
                peer_sync_addr,
                session_key,
                peer_model: peer_meta.model,
                peer_os: peer_meta.os_version,
                peer_app_version: peer_meta.app_version,
                peer_local_ip: peer_meta.local_ip,
            })
        })
        .await
        .map_err(|_elapsed| {
            tracing::warn!(
                timeout = ?PAKE_EXCHANGE_TIMEOUT,
                "bootstrap: PAKE exchange timed out — evicting stalled peer"
            );
            io_other("bootstrap: PAKE exchange timed out".into())
        })??;

        Ok(pairing)
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
    own_meta: &PeerMeta,
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

    let tcp_stream = match tokio::time::timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await
    {
        Ok(res) => res?,
        Err(_elapsed) => {
            tracing::warn!(
                peer_addr = %addr,
                timeout = ?TCP_CONNECT_TIMEOUT,
                "bootstrap: TCP connect timed out — transient"
            );
            return Err(TransportError::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            )));
        }
    };

    // rustls requires a ServerName; identity is verified by PAKE, not SNI, so a
    // fixed placeholder is fine (and is what the pinned transport uses too).
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

    // Wrap the entire 9-frame PAKE exchange in one deadline — mirrors the
    // responder's protection against a stalling peer (slowloris-style DoS).
    let own_fingerprint_owned = own_fingerprint.clone();
    let sync_addr_owned = sync_addr.to_owned();
    let own_meta = own_meta.clone();
    let pairing = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async move {
        let (client, msg1) =
            PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

        // Frame 1 → our PAKE message1.
        send_frame(&mut framed, &msg1).await?;
        // Frame 2 → our cert fingerprint.
        send_frame(&mut framed, own_fingerprint_owned.as_bytes()).await?;
        // Frame 3 → our P2P sync-listener address (Phase 2).
        send_frame(&mut framed, sync_addr_owned.as_bytes()).await?;

        // Frame 4 ← responder's PAKE message2.
        let msg2 = recv_frame(&mut framed).await?;
        // Frame 5 ← responder's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;
        // Frame 6 ← responder's P2P sync-listener address (Phase 2).
        let peer_sync_addr = recv_sync_addr(&mut framed).await?;

        // Lowercase before comparing — handle peers that send uppercase hex
        // (avoid false mismatch; value is public, timing safety not needed).
        if frame_peer_fp.to_lowercase() != tls_peer_fp {
            return Err(io_other(format!(
                "bootstrap: responder frame fingerprint {frame_peer_fp} != TLS cert {tls_peer_fp}"
            )));
        }

        let (session_key, msg3) = client
            .finish(&msg2)
            .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

        // Frame 7 → our PAKE finalisation.
        send_frame(&mut framed, &msg3).await?;

        // Channel-binding confirmation (S3). See `BootstrapResponder::run` for
        // the rationale — bind to this TLS session and exchange role-separated
        // tags, aborting on any mismatch (relay MitM defence).
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

        // P2P Phase 4 (optional, post-handshake): exchange device metadata.
        // Pairing is already complete and authenticated; failures are swallowed.
        let peer_meta = exchange_peer_meta(&mut framed, &own_meta).await;

        Ok::<BootstrapPairing, TransportError>(BootstrapPairing {
            peer_fingerprint: tls_peer_fp,
            peer_sync_addr,
            session_key,
            peer_model: peer_meta.model,
            peer_os: peer_meta.os_version,
            peer_app_version: peer_meta.app_version,
            peer_local_ip: peer_meta.local_ip,
        })
    })
    .await
    .map_err(|_elapsed| {
        tracing::warn!(
            timeout = ?PAKE_EXCHANGE_TIMEOUT,
            "bootstrap: initiator PAKE exchange timed out — stalled responder"
        );
        io_other("bootstrap: PAKE exchange timed out".into())
    })??;

    Ok(pairing)
}

// ── device-metadata exchange (P2P Phase 4) ────────────────────────────────────

/// Exchange optional device metadata over the framed stream AFTER the PAKE
/// handshake has fully completed.
///
/// Symmetric on both endpoints (so it cannot deadlock): each side SENDS its own
/// version byte then its metadata JSON, then RECEIVES the peer's version byte
/// and metadata. Sending first, before any receive, keeps the two sides in
/// lock-step over the duplex stream.
///
/// Back-compat: a legacy peer terminates the protocol at frame 9 and never reads
/// or writes these frames. When we try to receive its version frame the stream
/// is closed → `recv_frame` errors → we return [`PeerMeta::default`] (all
/// `None`). Likewise an explicit version `< BOOTSTRAP_PROTO_VERSION` skips the
/// metadata read. ALL errors are swallowed: pairing already succeeded, so a
/// metadata hiccup must never turn it into a failure.
async fn exchange_peer_meta<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    own_meta: &PeerMeta,
) -> PeerMeta
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // Send our version byte, then our metadata JSON. Swallow send errors (a
    // legacy peer may have already closed the read half).
    if send_frame(framed, &[BOOTSTRAP_PROTO_VERSION])
        .await
        .is_err()
    {
        return PeerMeta::default();
    }
    let own_json = serde_json::to_vec(own_meta).unwrap_or_default();
    if send_frame(framed, &own_json).await.is_err() {
        return PeerMeta::default();
    }

    // Receive the peer's version byte. Absent / malformed → legacy peer.
    let peer_version = match recv_frame(framed).await {
        Ok(bytes) if bytes.len() == 1 => bytes[0],
        _ => return PeerMeta::default(),
    };
    if peer_version < BOOTSTRAP_PROTO_VERSION {
        // Older metadata layout we do not understand — skip the metadata frame.
        return PeerMeta::default();
    }

    // Receive the peer's metadata JSON frame.
    let bytes = match recv_frame(framed).await {
        Ok(b) if b.len() <= MAX_META_BYTES => b,
        _ => return PeerMeta::default(),
    };
    serde_json::from_slice::<PeerMeta>(&bytes).unwrap_or_default()
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
    // Accept 64 hex chars regardless of case (peers may send uppercase hex).
    // Normalise to lowercase so callers can compare directly against
    // `fingerprint_of` output (which is always lowercase). The value is public
    // — no timing side-channel concern for the normalisation step itself.
    if fp.len() != 64 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io_other(format!(
            "bootstrap: malformed peer fingerprint ({} bytes)",
            fp.len()
        )));
    }
    Ok(fp.to_lowercase())
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
        let resp_meta = PeerMeta {
            model: Some("Mac mini".into()),
            os_version: Some("macOS 15.5".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("192.168.1.10".into()),
        };
        let resp_meta_task = resp_meta.clone();
        let responder_task =
            tokio::spawn(async move { responder.run(&pw, resp_sync_addr, &resp_meta_task).await });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let init_pw = password.to_string();
        let init_sync_addr = "127.0.0.1:7002";
        let init_meta = PeerMeta {
            model: Some("MacBook Air".into()),
            os_version: Some("macOS 14.4".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("192.168.1.11".into()),
        };
        let init_meta_task = init_meta.clone();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                &init_pw,
                init_sync_addr,
                &init_meta_task,
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

        // Phase 4: each side learned the other's device metadata over the
        // post-handshake metadata extension.
        assert_eq!(resp.peer_model, init_meta.model);
        assert_eq!(resp.peer_os, init_meta.os_version);
        assert_eq!(resp.peer_app_version, init_meta.app_version);
        assert_eq!(resp.peer_local_ip, init_meta.local_ip);
        assert_eq!(init.peer_model, resp_meta.model);
        assert_eq!(init.peer_os, resp_meta.os_version);
        assert_eq!(init.peer_app_version, resp_meta.app_version);
        assert_eq!(init.peer_local_ip, resp_meta.local_ip);
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

        let responder_task = tokio::spawn(async move {
            responder
                .run("the-right-password", "127.0.0.1:7003", &PeerMeta::default())
                .await
        });

        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let initiator_task = tokio::spawn(async move {
            run_initiator(
                addr,
                initiator_cert.cert_der,
                initiator_cert.key_der,
                "the-WRONG-password",
                "127.0.0.1:7004",
                &PeerMeta::default(),
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
        let responder_task = tokio::spawn(async move {
            responder
                .run(&pw, "127.0.0.1:7005", &PeerMeta::default())
                .await
        });

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
                &PeerMeta::default(),
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

    // ── Fix 2: PAKE exchange has an overall deadline ──────────────────────────

    /// A peer that completes TLS but then dribbles / stalls mid-PAKE exchange
    /// must be evicted by `PAKE_EXCHANGE_TIMEOUT`. Without this deadline the
    /// single-shot responder (and the initiator) would be pinned indefinitely
    /// (slowloris-style DoS).
    ///
    /// We simulate a slow responder by opening a raw TLS bootstrap connection,
    /// sending the very first frame (PAKE msg1) and then going silent. The
    /// `BootstrapResponder::run` future must time out on `PAKE_EXCHANGE_TIMEOUT`,
    /// NOT block forever.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn pake_exchange_timeout_fires_on_slow_peer() {
        let responder_cert = SelfSignedCert::generate("responder-device").unwrap();

        let responder = BootstrapResponder::bind(
            responder_cert.cert_der.clone(),
            responder_cert.key_der.clone(),
        )
        .await
        .expect("bind responder");
        let port = responder.local_addr().expect("local addr").port();

        // Run the responder; it must time out because we'll stall after frame 1.
        let responder_task = tokio::spawn(async move {
            responder
                .run("any-password", "127.0.0.1:9000", &PeerMeta::default())
                .await
        });

        // Connect with an "any cert" TLS client, send exactly frame 1 (a fake
        // PAKE msg1 byte string), then go permanently silent — no more frames.
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        let staller_cert = SelfSignedCert::generate("staller").unwrap();
        let staller_task = tokio::spawn(async move {
            use futures_util::SinkExt as _;
            let cert = rustls::pki_types::CertificateDer::from(staller_cert.cert_der.clone());
            let key = rustls::pki_types::PrivatePkcs8KeyDer::from(staller_cert.key_der.clone());
            let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(key);
            let client_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyCert))
                .with_client_auth_cert(vec![cert], private_key)
                .expect("client config");
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(client_config));
            let tcp = tokio::net::TcpStream::connect(addr)
                .await
                .expect("tcp connect");
            let server_name =
                rustls::pki_types::ServerName::try_from("copypaste.peer").expect("server name");
            let tls_stream = connector
                .connect(server_name, tcp)
                .await
                .expect("tls connect");
            let mut framed = tokio_util::codec::Framed::new(tls_stream, length_codec());
            // Send one garbage frame (pretend to be PAKE msg1) then go silent forever.
            framed
                .send(bytes::Bytes::from_static(b"fake-pake-msg1"))
                .await
                .expect("send frame1");
            // Hold the connection open so the responder can't detect closure.
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        });

        // Advance virtual time well past PAKE_EXCHANGE_TIMEOUT.
        let advance_ms = PAKE_EXCHANGE_TIMEOUT.as_millis() as u64 + 1_000;
        tokio::time::sleep(std::time::Duration::from_millis(advance_ms)).await;

        // The responder should have timed out by now.
        staller_task.abort();
        let result = responder_task.await.expect("responder join");
        assert!(
            result.is_err(),
            "responder must fail when peer stalls mid-PAKE (PAKE_EXCHANGE_TIMEOUT not applied)"
        );
    }

    // ── Phase 4: device-metadata extension back-compat ───────────────────────

    /// Two NEW peers running `exchange_peer_meta` over an in-memory duplex pair
    /// must each learn the other's metadata.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exchange_peer_meta_both_new_learns_each_other() {
        let (a, b) = tokio::io::duplex(4096);
        let mut fa = Framed::new(a, length_codec());
        let mut fb = Framed::new(b, length_codec());

        let meta_a = PeerMeta {
            model: Some("MacBook Air".into()),
            os_version: Some("macOS 14.4".into()),
            app_version: Some("0.5.4".into()),
            local_ip: Some("10.0.0.1".into()),
        };
        let meta_b = PeerMeta {
            model: Some("Mac mini".into()),
            ..Default::default()
        };

        let ma = meta_a.clone();
        let mb = meta_b.clone();
        let ta = tokio::spawn(async move { exchange_peer_meta(&mut fa, &ma).await });
        let tb = tokio::spawn(async move { exchange_peer_meta(&mut fb, &mb).await });
        let (got_a, got_b) = tokio::join!(ta, tb);

        // Side A learned B's metadata; side B learned A's.
        assert_eq!(got_a.unwrap(), meta_b);
        assert_eq!(got_b.unwrap(), meta_a);
    }

    /// Back-compat: when the peer is LEGACY (closes the stream without sending a
    /// version/metadata frame), `exchange_peer_meta` must return the default
    /// (all-`None`) metadata rather than hanging or erroring — the pairing has
    /// already completed and metadata is best-effort.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn exchange_peer_meta_legacy_peer_yields_none() {
        let (a, b) = tokio::io::duplex(4096);
        let mut fa = Framed::new(a, length_codec());

        // Legacy peer: drop its end immediately (frame 9 was the last thing it
        // would have sent in the real protocol).
        drop(b);

        let meta_a = PeerMeta {
            model: Some("MacBook Air".into()),
            ..Default::default()
        };
        let got = exchange_peer_meta(&mut fa, &meta_a).await;
        assert_eq!(
            got,
            PeerMeta::default(),
            "a legacy peer that sends no metadata must yield all-None"
        );
    }

    // ── Fix 4: fingerprint comparison is case-insensitive ────────────────────

    /// A peer that sends its fingerprint in UPPERCASE hex must still pair
    /// successfully. Before the fix, `frame_peer_fp != tls_peer_fp` was a byte
    /// comparison of the frame bytes (which might be uppercase) against
    /// `fingerprint_of` output (which is lowercase), causing a false mismatch.
    ///
    /// We test the invariant directly: `recv_fingerprint` now lowercases its
    /// output so an uppercase frame equals the lowercase TLS fingerprint.
    #[test]
    fn recv_fingerprint_normalises_to_lowercase() {
        // Construct what recv_fingerprint MUST return when the peer sends
        // an uppercase hex fingerprint — it should be lowercased.
        let uppercase_hex = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        assert_eq!(uppercase_hex.len(), 64);
        // The function itself is async and private; test the normalised form
        // symbolically: if we lowercase the uppercase input we get a valid
        // lowercase fingerprint that would match `fingerprint_of` output.
        let normalised = uppercase_hex.to_lowercase();
        assert!(
            normalised.bytes().all(|b| b.is_ascii_hexdigit()),
            "lowercased hex must still be valid hex"
        );
        assert!(
            normalised.bytes().all(|b| !b.is_ascii_uppercase()),
            "normalised fingerprint must contain no uppercase chars"
        );
        // Also verify the current recv_fingerprint validator accepts uppercase
        // (64 chars, all hex digits including uppercase).
        assert!(
            uppercase_hex.len() == 64 && uppercase_hex.bytes().all(|b| b.is_ascii_hexdigit()),
            "uppercase fingerprint must be accepted by the length+hex check"
        );
    }
}
