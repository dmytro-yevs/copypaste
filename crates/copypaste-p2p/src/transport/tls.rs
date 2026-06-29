//! RFC 5705 TLS channel-binding export helpers.

use tokio::net::TcpStream;

use super::connection::TransportError;

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
