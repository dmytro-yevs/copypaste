//! Bootstrap TLS responder — server-side PAKE handshake for P2P pairing.

use std::net::SocketAddr;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_util::codec::Framed;

use super::framing::{
    io_other, length_codec, recv_confirm_byte, recv_confirmation_tag, recv_fingerprint, recv_frame,
    recv_sync_addr, send_frame, SAS_ACCEPT, SAS_REJECT,
};
use super::meta::exchange_peer_meta;
use super::tls::AcceptAnyCert;
use super::types::{BootstrapPairing, PeerMeta, SyncProvisioning};
use crate::bootstrap::{BOOTSTRAP_ACCEPT_TIMEOUT, PAKE_EXCHANGE_TIMEOUT};
use crate::cert::fingerprint_of;
use crate::pake::{channel_confirmation_tag, derive_sas, ConfirmRole, PakeResponder, PasswordFile};
use crate::transport::{tls_channel_binder_server, TransportError, TLS_HANDSHAKE_TIMEOUT};

/// A bootstrap TLS responder listener bound to an ephemeral port.
///
/// Construct with [`BootstrapResponder::bind`], read [`local_addr`](Self::local_addr)
/// into the QR `addr_hint`, then call [`BootstrapResponder::run`] to accept one
/// connection and drive the responder side of the PAKE handshake over it.
pub struct BootstrapResponder {
    pub(super) listener: TcpListener,
    pub(super) acceptor: TlsAcceptor,
    pub(super) own_cert_der: Vec<u8>,
    pub(super) own_fingerprint: crate::transport::DeviceFingerprint,
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
        Self::bind_on(0, cert_der, key_der).await
    }

    /// Bind the bootstrap listener on a SPECIFIC TCP port (`0.0.0.0:port`) and
    /// TLS-wrap it with the daemon's self-signed certificate.
    ///
    /// `port = 0` requests an OS-assigned ephemeral port (the QR path's
    /// behaviour, via [`bind`](Self::bind)). The LAN/SAS Phase 2 standing
    /// responder uses a FIXED port so the advertised mDNS `bport` stays stable
    /// across pairing iterations: it discovers a free port once, advertises it,
    /// then re-binds the SAME port for each subsequent inbound pairing (a
    /// listening socket is dropped — not connected — so it never enters
    /// TIME_WAIT and re-bind succeeds immediately).
    ///
    /// # Errors
    /// Returns [`TransportError::Io`] if the bind fails or
    /// [`TransportError::TlsConfig`] if the TLS config cannot be built.
    pub async fn bind_on(
        port: u16,
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
    ) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
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
        own_provisioning: Option<SyncProvisioning>,
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

            // SAS for the human compare (LAN/SAS path). Additive: computing it
            // here does NOT change the wire transcript — the legacy path simply
            // surfaces it in the returned struct without exchanging frame 10a.
            let sas = derive_sas(&bound_key);

            // P2P Phase 4 (optional, post-handshake): exchange device metadata
            // and (proto >= 2) sync provisioning. The pairing is already complete
            // and authenticated at this point; any failure here is swallowed
            // (legacy peer closed, etc.).
            let (peer_meta, peer_provisioning) =
                exchange_peer_meta(&mut framed, &own_meta, own_provisioning.as_ref()).await;

            Ok::<BootstrapPairing, TransportError>(BootstrapPairing {
                peer_fingerprint: tls_peer_fp,
                peer_sync_addr,
                session_key,
                sas,
                peer_model: peer_meta.model,
                peer_os: peer_meta.os_version,
                peer_app_version: peer_meta.app_version,
                peer_local_ip: peer_meta.local_ip,
                peer_device_name: peer_meta.device_name,
                peer_public_ip: peer_meta.public_ip,
                peer_device_id: peer_meta.device_id,
                peer_provisioning,
                peer_supabase_account_id: peer_meta.supabase_account_id,
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

    /// Confirm-gated variant of [`BootstrapResponder::run`] for the LAN/SAS
    /// discovery pairing path.
    ///
    /// Runs the IDENTICAL handshake transcript through frame 9 (PAKE +
    /// channel-binding tag verify), then derives the 6-digit SAS and invokes
    /// `confirm(sas, peer_fingerprint)`. If the user rejects (returns `false`) the
    /// pairing aborts with an error (keys drop/zeroize). Otherwise both sides
    /// exchange a NEW frame 10a (`SAS_ACCEPT`/`SAS_REJECT`) and proceed to
    /// the metadata exchange / `Ok` ONLY if BOTH bytes are `SAS_ACCEPT`.
    ///
    /// The `peer_fingerprint` argument is the TLS peer certificate fingerprint
    /// observed during the bootstrap handshake (the same value stored in
    /// `BootstrapPairing::peer_fingerprint`). Passing it to the callback gives
    /// the daemon coordinator identity binding on the responder path — matching
    /// what the initiator path already has (CopyPaste-n3bc).
    ///
    /// This is a separate method so the QR `run` transcript stays byte-compatible
    /// (frame 10a is never sent there).
    #[allow(clippy::too_many_arguments)] // mirrors `run` + confirm cb + provisioning
    pub async fn run_with_confirm<F, Fut>(
        self,
        password: &str,
        sync_addr: &str,
        own_meta: &PeerMeta,
        own_provisioning: Option<SyncProvisioning>,
        confirm: F,
    ) -> Result<BootstrapPairing, TransportError>
    where
        F: FnOnce(&str, &str) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let (tcp_stream, peer_addr) =
            match tokio::time::timeout(BOOTSTRAP_ACCEPT_TIMEOUT, self.listener.accept()).await {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout = ?BOOTSTRAP_ACCEPT_TIMEOUT,
                        "bootstrap: SAS responder timed out waiting for inbound pairing connection"
                    );
                    return Err(TransportError::HandshakeTimeout);
                }
            };
        tracing::debug!(peer_addr = %peer_addr, "bootstrap(sas): inbound TCP connection");

        let tls_stream =
            match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, self.acceptor.accept(tcp_stream))
                .await
            {
                Ok(res) => res?,
                Err(_elapsed) => {
                    tracing::warn!("bootstrap(sas): TLS server handshake timed out");
                    return Err(TransportError::HandshakeTimeout);
                }
            };

        let tls_peer_fp = {
            let (_, conn) = tls_stream.get_ref();
            let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
            let first = certs.first().ok_or(TransportError::NoPeerCert)?;
            fingerprint_of(first.as_ref())
        };

        let tls_binder = tls_channel_binder_server(&tls_stream)?;
        let mut framed = Framed::new(tls_stream, length_codec());
        debug_assert!(!self.own_cert_der.is_empty());

        let own_fingerprint = self.own_fingerprint.clone();

        // The 9-frame PAKE exchange is bounded by PAKE_EXCHANGE_TIMEOUT. It
        // borrows `framed` (so it can be reused for frame 10a) and returns the
        // SAS plus everything needed to finish. The user-confirm step runs
        // OUTSIDE this deadline: a human may take longer than 30 s, and a slow
        // confirm must not be mistaken for a stalled peer.
        let prepared = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async {
            let password_file = PasswordFile::register(password)
                .map_err(|e| io_other(format!("PasswordFile::register: {e}")))?;

            // Frame 1 ← initiator's PAKE message1.
            let msg1 = recv_frame(&mut framed).await?;
            // Frame 2 ← initiator's cert fingerprint.
            let frame_peer_fp = recv_fingerprint(&mut framed).await?;
            // Frame 3 ← initiator's P2P sync-listener address.
            let peer_sync_addr = recv_sync_addr(&mut framed).await?;

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
            // Frame 6 → our P2P sync-listener address.
            send_frame(&mut framed, sync_addr.as_bytes()).await?;

            // Frame 7 ← initiator's PAKE finalisation.
            let msg3 = recv_frame(&mut framed).await?;
            let session_key = responder
                .finish(&msg3)
                .map_err(|e| io_other(format!("PAKE finish: {e}")))?;

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

            let sas = derive_sas(&bound_key);
            Ok::<_, TransportError>((sas, tls_peer_fp, peer_sync_addr, session_key))
        })
        .await
        .map_err(|_elapsed| {
            tracing::warn!(
                timeout = ?PAKE_EXCHANGE_TIMEOUT,
                "bootstrap(sas): PAKE exchange timed out — evicting stalled peer"
            );
            io_other("bootstrap: PAKE exchange timed out".into())
        })??;

        let (sas, peer_fingerprint, peer_sync_addr, session_key) = prepared;

        // Human SAS confirmation (outside the PAKE deadline). On reject, return
        // an error so `session_key` drops/zeroizes and the caller persists nothing.
        // CopyPaste-n3bc: pass peer_fingerprint alongside sas so the daemon
        // coordinator has identity binding on the responder path.
        let accepted_locally = confirm(&sas, &peer_fingerprint).await;

        // Frame 10a: exchange ACCEPT/REJECT bytes. Proceed only if BOTH accept.
        let our_byte = if accepted_locally {
            SAS_ACCEPT
        } else {
            SAS_REJECT
        };
        send_frame(&mut framed, &[our_byte]).await?;
        let peer_byte = recv_confirm_byte(&mut framed).await?;
        if our_byte != SAS_ACCEPT || peer_byte != SAS_ACCEPT {
            return Err(io_other("SAS rejected by user — pairing aborted".into()));
        }

        // Both confirmed: optional post-handshake metadata + provisioning.
        let (peer_meta, peer_provisioning) =
            exchange_peer_meta(&mut framed, own_meta, own_provisioning.as_ref()).await;

        Ok(BootstrapPairing {
            peer_fingerprint,
            peer_sync_addr,
            session_key,
            sas,
            peer_model: peer_meta.model,
            peer_os: peer_meta.os_version,
            peer_app_version: peer_meta.app_version,
            peer_local_ip: peer_meta.local_ip,
            peer_device_name: peer_meta.device_name,
            peer_public_ip: peer_meta.public_ip,
            peer_device_id: peer_meta.device_id,
            peer_provisioning,
            peer_supabase_account_id: peer_meta.supabase_account_id,
        })
    }
}
