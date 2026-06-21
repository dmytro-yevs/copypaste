//! Bootstrap initiator — client-side PAKE handshake for P2P pairing.

use std::net::SocketAddr;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::ClientConfig;
use subtle::ConstantTimeEq;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_util::codec::Framed;

use super::framing::{
    io_other, length_codec, recv_confirm_byte, recv_confirmation_tag, recv_fingerprint, recv_frame,
    recv_sync_addr, send_frame, SAS_ACCEPT, SAS_REJECT,
};
use super::meta::exchange_peer_meta;
use super::tls::AcceptAnyCert;
use super::types::{BootstrapPairing, PeerMeta, SyncProvisioning};
use crate::bootstrap::PAKE_EXCHANGE_TIMEOUT;
use crate::cert::fingerprint_of;
use crate::pake::{channel_confirmation_tag, derive_sas, ConfirmRole, PakeInitiator};
use crate::transport::{
    tls_channel_binder_client, TransportError, P2P_SNI_SENTINEL, TCP_CONNECT_TIMEOUT,
    TLS_HANDSHAKE_TIMEOUT,
};

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
/// Mirrors [`crate::bootstrap::BootstrapResponder::run`] — TLS / socket / framing errors and PAKE
/// failures (including a wrong password, surfaced from `client.finish`).
#[allow(clippy::too_many_arguments)] // additive provisioning param mirrors `run`
pub async fn run_initiator(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    password: &str,
    sync_addr: &str,
    own_meta: &PeerMeta,
    own_provisioning: Option<SyncProvisioning>,
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

        // SAS for the human compare (LAN/SAS path). Additive — does not change
        // the wire transcript; the legacy path just surfaces it.
        let sas = derive_sas(&bound_key);

        // P2P Phase 4 (optional, post-handshake): exchange device metadata and
        // (proto >= 2) sync provisioning. Pairing is already complete and
        // authenticated; failures are swallowed.
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

/// Confirm-gated variant of [`run_initiator`] for the LAN/SAS discovery pairing
/// path.
///
/// Runs the IDENTICAL handshake transcript through frame 9 (PAKE +
/// channel-binding tag verify), then derives the 6-digit SAS and invokes
/// `confirm(sas, peer_fingerprint)`. On reject (`false`) the pairing aborts
/// with an error so the session key drops/zeroizes. Otherwise both sides exchange
/// frame 10a (`SAS_ACCEPT`/`SAS_REJECT`) and the pairing succeeds ONLY if
/// BOTH bytes are `SAS_ACCEPT`.
///
/// The `peer_fingerprint` argument is the TLS peer certificate fingerprint
/// observed during the bootstrap handshake (the same value stored in
/// `BootstrapPairing::peer_fingerprint`). Passing it to the callback gives the
/// daemon coordinator identity binding on both paths (CopyPaste-n3bc).
///
/// Separate from [`run_initiator`] so the QR transcript stays byte-compatible.
#[allow(clippy::too_many_arguments)] // mirrors `run_initiator` + confirm cb + provisioning
pub async fn run_initiator_with_confirm<F, Fut>(
    addr: SocketAddr,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
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
                "bootstrap(sas): TCP connect timed out — transient"
            );
            return Err(TransportError::Io(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            )));
        }
    };

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
            tracing::warn!(peer_addr = %addr, "bootstrap(sas): TLS client handshake timed out");
            return Err(TransportError::HandshakeTimeout);
        }
    };

    let tls_peer_fp = {
        let (_, conn) = tls_stream.get_ref();
        let certs = conn.peer_certificates().ok_or(TransportError::NoPeerCert)?;
        let first = certs.first().ok_or(TransportError::NoPeerCert)?;
        fingerprint_of(first.as_ref())
    };

    let tls_binder = tls_channel_binder_client(&tls_stream)?;
    let mut framed = Framed::new(tls_stream, length_codec());

    let own_fingerprint_owned = own_fingerprint.clone();

    // 9-frame exchange bounded by PAKE_EXCHANGE_TIMEOUT; borrows `framed` so it
    // is reusable for frame 10a. Confirm runs OUTSIDE this deadline.
    let prepared = tokio::time::timeout(PAKE_EXCHANGE_TIMEOUT, async {
        let (client, msg1) =
            PakeInitiator::new(password).map_err(|e| io_other(format!("PAKE init: {e}")))?;

        // Frame 1 → our PAKE message1.
        send_frame(&mut framed, &msg1).await?;
        // Frame 2 → our cert fingerprint.
        send_frame(&mut framed, own_fingerprint_owned.as_bytes()).await?;
        // Frame 3 → our P2P sync-listener address.
        send_frame(&mut framed, sync_addr.as_bytes()).await?;

        // Frame 4 ← responder's PAKE message2.
        let msg2 = recv_frame(&mut framed).await?;
        // Frame 5 ← responder's cert fingerprint.
        let frame_peer_fp = recv_fingerprint(&mut framed).await?;
        // Frame 6 ← responder's P2P sync-listener address.
        let peer_sync_addr = recv_sync_addr(&mut framed).await?;

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

        let sas = derive_sas(&bound_key);
        Ok::<_, TransportError>((sas, tls_peer_fp, peer_sync_addr, session_key))
    })
    .await
    .map_err(|_elapsed| {
        tracing::warn!(
            timeout = ?PAKE_EXCHANGE_TIMEOUT,
            "bootstrap(sas): initiator PAKE exchange timed out — stalled responder"
        );
        io_other("bootstrap: PAKE exchange timed out".into())
    })??;

    let (sas, peer_fingerprint, peer_sync_addr, session_key) = prepared;

    // Human SAS confirmation (outside the PAKE deadline). Reject → error → keys
    // drop/zeroize.
    // CopyPaste-n3bc: pass peer_fingerprint alongside sas so the daemon
    // coordinator has identity binding on the initiator path too.
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
    })
}
