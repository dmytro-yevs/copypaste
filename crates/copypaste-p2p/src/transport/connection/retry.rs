//! Transient-vs-permanent [`TransportError`] classification for
//! [`super::PeerTransport::connect_with_retry`], plus the peer-certificate
//! fingerprint extraction used by [`super::PeerTransport::accept`].

use tokio::net::TcpStream;

use crate::cert::fingerprint_of;

use super::error::TransportError;

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
pub(super) fn is_transient_transport_error(err: &TransportError) -> bool {
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
pub(super) fn is_transient_io_kind(kind: std::io::ErrorKind) -> bool {
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

pub(super) fn peer_fingerprint_server(
    stream: &tokio_rustls::server::TlsStream<TcpStream>,
) -> Result<String, TransportError> {
    let (_, server_conn) = stream.get_ref();
    let certs = server_conn
        .peer_certificates()
        .ok_or(TransportError::NoPeerCert)?;
    let first = certs.first().ok_or(TransportError::NoPeerCert)?;
    Ok(fingerprint_of(first.as_ref()))
}
