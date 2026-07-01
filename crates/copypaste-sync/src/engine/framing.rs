//! Length-prefixed JSON frame send/recv over `AsyncRead + AsyncWrite`.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::error::SyncError;
use crate::protocol::Message;

/// Maximum number of bytes allowed in a single protocol frame (16 MiB).
///
/// This is the canonical value shared by the P2P data-plane transport
/// (`copypaste_p2p::transport::MAX_FRAME_BYTES`).  Both sites must remain equal;
/// a compile-time equality assertion lives in
/// `copypaste-daemon/tests/frame_consts.rs` (CopyPaste-w47w #1).
///
/// Exported as `usize` so the value is usable by codec builders; the internal
/// `MAX_FRAME_SIZE` helper below casts it to `u32` for the hand-written
/// 4-byte length-prefix framing used by `recv_message`.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// `u32` variant of [`MAX_FRAME_BYTES`] used internally for the 4-byte
/// little-endian length prefix.  The cast is infallible: 16 MiB fits in u32.
///
/// `pub(crate)` (rather than file-private) solely so `engine::tests` can reach
/// it via `use super::*` after the module split — not a public-API widening.
pub(crate) const MAX_FRAME_SIZE: u32 = MAX_FRAME_BYTES as u32;

/// Send a protocol message as a length-prefixed JSON frame.
pub(crate) async fn send_message<S: AsyncWrite + Unpin>(
    stream: &mut S,
    msg: &Message,
) -> Result<(), SyncError> {
    let frame = msg.encode()?;
    stream.write_all(&frame).await?;
    Ok(())
}

/// Read the next length-prefixed JSON frame and deserialise it.
pub(crate) async fn recv_message<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Message, SyncError> {
    // Read 4-byte length prefix.
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);

    if len > MAX_FRAME_SIZE {
        return Err(SyncError::FrameTooLarge(len));
    }

    // Read payload.
    let mut payload = vec![0u8; len as usize];
    stream.read_exact(&mut payload).await?;

    let msg = Message::decode(&payload)?;
    Ok(msg)
}
