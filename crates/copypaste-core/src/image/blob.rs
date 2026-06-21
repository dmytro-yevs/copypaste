//! BLOB serialization: `EncryptedChunk` ↔ flat SQLite BLOB.
//!
//! Format: `[chunk_count: u32 BE] [chunk_0_wire] [chunk_1_wire] ...`
//! where each `chunk_N_wire` is prefixed by its wire length as `u32 BE`.

use crate::crypto::chunks::{EncryptedChunk, CHUNK_FORMAT_VERSION};

use super::ImageError;
use crate::crypto::chunks::ChunkError;

/// Serialize chunks to a flat byte blob for SQLite BLOB storage.
///
/// Format: `[chunk_count: u32 BE] [chunk_0_wire] [chunk_1_wire] ...`
///
/// Returns `Err(ImageError::Chunk(ChunkError::TooManyChunks))` if the slice
/// is somehow longer than `u32::MAX` (cannot happen via `encrypt_chunks` which
/// enforces the same bound, but avoids a panic on a direct call with an
/// oversized slice).
pub fn chunks_to_blob(chunks: &[EncryptedChunk]) -> Result<Vec<u8>, ImageError> {
    let count =
        u32::try_from(chunks.len()).map_err(|_| ImageError::Chunk(ChunkError::TooManyChunks))?;
    // F3: pre-size the output so the repeated `extend_from_slice` below never
    // reallocs (which, for a multi-MiB blob, spikes peak memory to ~2x). The
    // exact layout is:
    //   4 (count) + Σ over chunks of [ 4 (wire-len prefix) + wire_len ]
    // where wire_len = 34 header bytes ([version:1][index:4][is_final:1]
    // [nonce:24][ct_len:4]) + ciphertext.len(). Computed from `ciphertext.len()`
    // directly so we do NOT allocate a throwaway `to_wire()` just to measure it.
    // `usize` math cannot overflow in practice: `count` fits in u32 and each
    // ciphertext is bounded by the chunk size, so the sum is far below usize::MAX.
    const WIRE_HEADER_LEN: usize = 1 + 4 + 1 + 24 + 4; // = 34
    let total: usize = 4 + chunks
        .iter()
        .map(|c| 4 + WIRE_HEADER_LEN + c.ciphertext.len())
        .sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&count.to_be_bytes());
    for chunk in chunks {
        let wire = chunk.to_wire();
        out.extend_from_slice(&(wire.len() as u32).to_be_bytes());
        out.extend_from_slice(&wire);
    }
    debug_assert_eq!(out.len(), total, "chunks_to_blob presize must be exact");
    Ok(out)
}

/// Deserialize chunks from the SQLite BLOB format produced by `chunks_to_blob`.
pub fn chunks_from_blob(blob: &[u8]) -> Result<Vec<EncryptedChunk>, ImageError> {
    if blob.len() < 4 {
        return Err(ImageError::Decode("blob too short".into()));
    }
    let count = u32::from_be_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;

    // Smallest possible per-chunk footprint in the blob: a 4-byte wire-length
    // prefix plus the minimum wire header [version:1][index:4][is_final:1]
    // [nonce:24][ct_len:4] = 34 bytes, i.e. 38 bytes total. A declared `count`
    // can therefore never exceed `(blob.len() - 4) / 38`. We clamp the reserve
    // against this ceiling so a corrupt/malicious blob with a huge count field
    // cannot trigger a multi-GB `Vec::with_capacity` allocation (OOM). The
    // per-chunk `pos` bounds checks below remain authoritative for correctness.
    const MIN_WIRE_CHUNK_LEN: usize = 4 + (1 + 4 + 1 + 24 + 4);
    let capacity_ceiling = (blob.len() - 4) / MIN_WIRE_CHUNK_LEN;
    let mut pos = 4usize;
    let mut chunks = Vec::with_capacity(count.min(capacity_ceiling));

    for _ in 0..count {
        if pos + 4 > blob.len() {
            return Err(ImageError::Decode("truncated blob (wire length)".into()));
        }
        let wire_len =
            u32::from_be_bytes([blob[pos], blob[pos + 1], blob[pos + 2], blob[pos + 3]]) as usize;
        pos += 4;

        if pos + wire_len > blob.len() {
            return Err(ImageError::Decode("truncated blob (wire data)".into()));
        }
        let wire = &blob[pos..pos + wire_len];
        pos += wire_len;

        // Parse wire format: [version:u8][index:u32][is_final:u8][nonce:24][len:u32][ciphertext]
        if wire.len() < 1 + 4 + 1 + 24 + 4 {
            return Err(ImageError::Decode("wire chunk too short".into()));
        }
        let version = wire[0];
        if version != CHUNK_FORMAT_VERSION {
            return Err(ImageError::Decode(format!(
                "unknown chunk version {version}"
            )));
        }
        let chunk_index = u32::from_be_bytes([wire[1], wire[2], wire[3], wire[4]]);
        let is_final = wire[5] != 0;
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&wire[6..30]);
        let ct_len = u32::from_be_bytes([wire[30], wire[31], wire[32], wire[33]]) as usize;
        if 34 + ct_len > wire.len() {
            return Err(ImageError::Decode("wire ciphertext truncated".into()));
        }
        let ciphertext = wire[34..34 + ct_len].to_vec();

        chunks.push(EncryptedChunk {
            chunk_index,
            is_final,
            nonce,
            ciphertext,
        });
    }

    Ok(chunks)
}
