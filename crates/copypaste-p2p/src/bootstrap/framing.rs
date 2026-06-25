//! Length-delimited framing helpers for the bootstrap wire protocol.
//!
//! All send/recv helpers are shared across the responder, initiator, and
//! metadata-exchange layers.

use futures_util::{SinkExt, StreamExt};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::pake::CONFIRM_TAG_LEN;
use crate::transport::TransportError;

/// Upper bound on a single PAKE/fingerprint bootstrap-handshake frame (64 KiB).
///
/// PAKE messages are a few hundred bytes and fingerprints are 64 hex chars;
/// 64 KiB is a wide margin that still rejects a desynced peer flooding a huge
/// length prefix.
///
/// This constant is INTENTIONALLY different from the data-plane cap
/// (`copypaste_p2p::transport::MAX_FRAME_BYTES` = 16 MiB): the bootstrap
/// handshake carries only small PAKE/fingerprint frames, not full clipboard
/// items.  The tighter bound provides defense-in-depth during pairing.
/// (CopyPaste-w47w #1 — do NOT merge this constant with the 16 MiB one.)
pub(super) const MAX_HANDSHAKE_FRAME_BYTES: usize = 64 * 1024;

/// Upper bound on the sync-provisioning JSON frame. Two URLs plus a base64-ish
/// anon key and a 32-byte key (base64 ≈ 44 chars) total well under 4 KiB; the
/// ceiling still rejects a desynced peer flooding this slot.
pub(super) const MAX_PROVISIONING_BYTES: usize = 4 * 1024;

/// SAS-confirm wire bytes (frame 10a, LAN/SAS pairing path only).
///
/// After frame 9 (channel-binding tag verified) the confirm-gated variants
/// exchange exactly one byte each: `SAS_ACCEPT` (0x01) when the local user
/// confirmed the SAS matched, or `SAS_REJECT` (0x00) otherwise. Pairing
/// proceeds to the metadata exchange / `Ok` ONLY when BOTH bytes are
/// `SAS_ACCEPT`. This frame exists solely on the new `*_with_confirm` paths;
/// the legacy `run`/`run_initiator` transcript is byte-unchanged.
pub(super) const SAS_ACCEPT: u8 = 0x01;
/// See [`SAS_ACCEPT`].
pub(super) const SAS_REJECT: u8 = 0x00;

/// Upper bound on the peer metadata JSON frame. The four short strings (model,
/// OS, app version, IP) total well under 256 bytes; 1 KiB is a wide ceiling that
/// still rejects a desynced peer flooding this slot.
pub(super) const MAX_META_BYTES: usize = 1024;

/// Upper bound on a peer's advertised sync-listener address. A `host:port` is at
/// most a few dozen bytes (IPv6 + port ≈ 47); 256 is a generous ceiling that
/// still rejects a desynced peer sending a huge frame in this slot.
pub(super) const MAX_SYNC_ADDR_BYTES: usize = 256;

pub(super) fn length_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_HANDSHAKE_FRAME_BYTES)
        .new_codec()
}

pub(super) fn io_other(msg: String) -> TransportError {
    TransportError::Io(std::io::Error::other(msg))
}

pub(super) async fn send_frame<S>(
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

pub(super) async fn recv_frame<S>(
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
pub(super) async fn recv_confirmation_tag<S>(
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

/// Receive the peer's frame-10a SAS-confirm byte (LAN/SAS path only).
///
/// Enforces an exact 1-byte frame so a desynced/malicious peer cannot smuggle a
/// longer frame into this slot. Any byte other than [`SAS_ACCEPT`] is treated as
/// a reject by the caller.
pub(super) async fn recv_confirm_byte<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
) -> Result<u8, TransportError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let bytes = recv_frame(framed).await?;
    if bytes.len() != 1 {
        return Err(io_other(format!(
            "bootstrap: SAS-confirm frame wrong length ({} bytes)",
            bytes.len()
        )));
    }
    Ok(bytes[0])
}

pub(super) async fn recv_fingerprint<S>(
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

/// Receive a peer's P2P sync-listener `host:port` address frame (Phase 2).
///
/// The address is opaque to the bootstrap layer (it is parsed/validated by the
/// daemon when it dials in Phase 3); this only enforces UTF-8 and a sane length
/// bound so a malformed frame cannot smuggle arbitrary bytes into `peers.json`.
/// An empty frame is accepted and returned as an empty string (the peer simply
/// did not advertise an address).
pub(super) async fn recv_sync_addr<S>(
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
