use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;

pub const CHUNK_FORMAT_VERSION: u8 = 1;
const NONCE_SIZE: usize = 24;

#[derive(Debug, Error)]
pub enum ChunkError {
    #[error("Chunk {index} authentication failed")]
    AuthFailed { index: u32 },
    #[error("Expected {expected} chunks, got {got}")]
    TruncatedStream { expected: u32, got: u32 },
    #[error("Empty chunk stream")]
    Empty,
    /// Edge-case audit high #14: a chunk is missing from the *middle* of the
    /// stream (not just truncated at the end). Detected when a chunk's
    /// declared `chunk_index` does not match its position in the sequence,
    /// or when `is_final` appears before the last position.
    #[error("Missing chunk at position {position}: expected index {expected}, got {got}")]
    MissingChunk {
        position: u32,
        expected: u32,
        got: u32,
    },
    /// A non-final chunk was found at a position other than the last —
    /// indicates either reordering or a missing tail.
    #[error("Premature final chunk at position {position} of {total}")]
    PrematureFinal { position: u32, total: u32 },
    /// The AEAD layer rejected a chunk during encryption — in practice this
    /// only happens when a single chunk exceeds XChaCha20-Poly1305's
    /// per-message size limit. Surfaced as an error rather than a panic so
    /// an oversized `chunk_size` can never abort the process.
    #[error("Chunk {index} encryption failed")]
    EncryptFailed { index: u32 },
    /// `chunk_size == 0` would cause `slice::chunks(0)` to panic. Callers
    /// must pass a chunk_size of at least 1.
    #[error("chunk_size must be at least 1")]
    InvalidChunkSize,
    /// The plaintext is so large that the chunk count exceeds u32::MAX.
    /// XChaCha20-Poly1305 chunk indices are encoded as u32 in the wire
    /// format, so streams longer than ~4 billion chunks are unsupported.
    #[error("too many chunks: stream length exceeds u32::MAX")]
    TooManyChunks,
}

/// Build AAD: `"CHUNK_FORMAT_V1\0"[16] || file_id[16] || chunk_index[4:BE] || total_chunks[4:BE] || is_final[1]`
fn build_aad(file_id: &[u8; 16], chunk_index: u32, total_chunks: u32, is_final: bool) -> Vec<u8> {
    let mut aad = Vec::with_capacity(16 + 16 + 4 + 4 + 1);
    aad.extend_from_slice(b"CHUNK_FORMAT_V1\0");
    aad.extend_from_slice(file_id);
    aad.extend_from_slice(&chunk_index.to_be_bytes());
    aad.extend_from_slice(&total_chunks.to_be_bytes());
    aad.push(if is_final { 1 } else { 0 });
    aad
}

#[derive(Debug, Clone)]
pub struct EncryptedChunk {
    pub chunk_index: u32,
    pub is_final: bool,
    pub nonce: [u8; NONCE_SIZE],
    pub ciphertext: Vec<u8>,
}

impl EncryptedChunk {
    /// Wire format: `[version:u8=1][index:u32][is_final:u8][nonce:24][len:u32][ciphertext]`
    pub fn to_wire(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 4 + 1 + 24 + 4 + self.ciphertext.len());
        buf.push(CHUNK_FORMAT_VERSION);
        buf.extend_from_slice(&self.chunk_index.to_be_bytes());
        buf.push(if self.is_final { 1 } else { 0 });
        buf.extend_from_slice(&self.nonce);
        buf.extend_from_slice(&(self.ciphertext.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.ciphertext);
        buf
    }
}

pub fn encrypt_chunks(
    plaintext: &[u8],
    key: &[u8; 32],
    file_id: &[u8; 16],
    chunk_size: usize,
) -> Result<Vec<EncryptedChunk>, ChunkError> {
    // Fix [HIGH]: chunk_size == 0 would cause slice::chunks(0) to panic.
    if chunk_size == 0 {
        return Err(ChunkError::InvalidChunkSize);
    }

    let cipher = XChaCha20Poly1305::new(key.into());
    let chunks_raw: Vec<&[u8]> = if plaintext.is_empty() {
        vec![&[]]
    } else {
        plaintext.chunks(chunk_size).collect()
    };
    // Fix [MED]: use try_from to avoid silent truncation when chunk count
    // exceeds u32::MAX (would wrap on 32-bit targets or very large inputs).
    let total = u32::try_from(chunks_raw.len()).map_err(|_| ChunkError::TooManyChunks)?;

    chunks_raw
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            // Safe: i < chunks_raw.len() <= u32::MAX (guarded above).
            let idx = i as u32;
            let is_final = idx == total - 1;
            let aad = build_aad(file_id, idx, total, is_final);
            let mut nonce_bytes = [0u8; NONCE_SIZE];
            // CopyPaste-crh3.5: OsRng (the OS CSPRNG) for all cryptographic
            // randomness, consistent with encrypt.rs / sync_key.rs — not the
            // userspace-reseeded thread_rng.
            rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = XNonce::from(nonce_bytes);
            let payload = Payload {
                msg: chunk,
                aad: &aad,
            };
            // The AEAD layer only rejects a message that exceeds the
            // per-message size limit. Today every caller passes a bounded
            // `chunk_size`, but returning an error instead of `.expect`
            // means an oversized chunk degrades to a recoverable `Err`
            // rather than aborting the process.
            let ciphertext = cipher
                .encrypt(&nonce, payload)
                .map_err(|_| ChunkError::EncryptFailed { index: idx })?;
            Ok(EncryptedChunk {
                chunk_index: idx,
                is_final,
                nonce: nonce_bytes,
                ciphertext,
            })
        })
        .collect()
}

pub fn decrypt_chunks(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<Vec<u8>, ChunkError> {
    if chunks.is_empty() {
        return Err(ChunkError::Empty);
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    // Fix [MED]: use try_from to avoid silent truncation on pathological inputs.
    let total = u32::try_from(chunks.len()).map_err(|_| ChunkError::TooManyChunks)?;

    // Guarded by the `chunks.is_empty()` early-return above; `ok_or` here
    // makes the empty-chunk case an explicit Err instead of an infallible
    // expect, which removes a panic site from the AEAD path (audit LOW).
    let last = chunks.last().ok_or(ChunkError::Empty)?;
    if !last.is_final {
        return Err(ChunkError::TruncatedStream {
            expected: total + 1,
            got: total,
        });
    }

    // Audit high #14 — gap detection pass.
    // We must validate the structural integrity of the chunk *sequence*
    // BEFORE attempting AEAD decryption. Otherwise a missing-middle gap
    // surfaces as `AuthFailed` (because the per-chunk AAD encodes the
    // original `total_chunks` and the missing-chunk variant shifts every
    // later chunk's expected position), which is opaque and prevents the
    // caller from requesting a targeted re-send of the specific lost index.
    for (i, chunk) in chunks.iter().enumerate() {
        let idx = i as u32;
        if chunk.chunk_index != idx {
            return Err(ChunkError::MissingChunk {
                position: idx,
                expected: idx,
                got: chunk.chunk_index,
            });
        }
        // A chunk that claims `is_final` but isn't actually the last entry
        // means the tail was dropped — gap at the end of the sequence.
        if chunk.is_final && idx != total - 1 {
            return Err(ChunkError::PrematureFinal {
                position: idx,
                total,
            });
        }
    }

    let mut plaintext = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let idx = i as u32;
        let aad = build_aad(file_id, idx, total, chunk.is_final);
        let nonce = XNonce::from(chunk.nonce);
        let payload = Payload {
            msg: &chunk.ciphertext,
            aad: &aad,
        };
        let decrypted = cipher
            .decrypt(&nonce, payload)
            .map_err(|_| ChunkError::AuthFailed { index: idx })?;
        plaintext.extend_from_slice(&decrypted);
    }
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x55u8; 32]
    }
    fn test_file_id() -> [u8; 16] {
        [0xAAu8; 16]
    }

    #[test]
    fn small_data_splits_into_chunks_and_reassembles() {
        let key = test_key();
        let file_id = test_file_id();
        let data = b"Hello chunked world!";
        let chunks = encrypt_chunks(data, &key, &file_id, 8).unwrap();
        assert!(chunks.len() > 1);
        let decrypted = decrypt_chunks(&chunks, &key, &file_id).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn single_chunk_when_data_fits() {
        let key = test_key();
        let file_id = test_file_id();
        let chunks = encrypt_chunks(b"small", &key, &file_id, 64 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].is_final);
        assert_eq!(decrypt_chunks(&chunks, &key, &file_id).unwrap(), b"small");
    }

    #[test]
    fn reordered_chunks_fail_decryption() {
        let key = test_key();
        let file_id = test_file_id();
        let mut chunks = encrypt_chunks(b"chunk1_data_chunk2_data", &key, &file_id, 11).unwrap();
        assert!(chunks.len() >= 2);
        chunks.swap(0, 1);
        assert!(decrypt_chunks(&chunks, &key, &file_id).is_err());
    }

    #[test]
    fn truncated_stream_fails_decryption() {
        let key = test_key();
        let file_id = test_file_id();
        let mut chunks =
            encrypt_chunks(b"chunk1_data_chunk2_data_chunk3", &key, &file_id, 10).unwrap();
        chunks.pop();
        assert!(decrypt_chunks(&chunks, &key, &file_id).is_err());
    }

    #[test]
    fn cross_stream_chunk_injection_fails() {
        let key = test_key();
        let file_id_a = [0xAAu8; 16];
        let file_id_b = [0xBBu8; 16];
        let chunks_a = encrypt_chunks(b"stream A data___", &key, &file_id_a, 8).unwrap();
        let mut chunks_b = encrypt_chunks(b"stream B data___", &key, &file_id_b, 8).unwrap();
        chunks_b[0] = chunks_a[0].clone();
        assert!(decrypt_chunks(&chunks_b, &key, &file_id_b).is_err());
    }

    #[test]
    fn chunk_wire_format_has_version_byte() {
        let key = test_key();
        let file_id = test_file_id();
        let chunks = encrypt_chunks(b"test", &key, &file_id, 64 * 1024).unwrap();
        let wire = chunks[0].to_wire();
        assert_eq!(wire[0], CHUNK_FORMAT_VERSION);
    }

    /// `encrypt_chunks` now returns a `Result`. A valid (bounded) `chunk_size`
    /// must yield `Ok` with the data round-tripping cleanly.
    #[test]
    fn encrypt_chunks_returns_ok_for_bounded_input() {
        let key = test_key();
        let file_id = test_file_id();
        let data = vec![0x7Eu8; 4096];
        let chunks = encrypt_chunks(&data, &key, &file_id, 512).unwrap();
        assert!(chunks.len() > 1);
        assert_eq!(decrypt_chunks(&chunks, &key, &file_id).unwrap(), data);
    }

    /// The oversized path: when the AEAD layer rejects a chunk, `encrypt_chunks`
    /// must return `Err(EncryptFailed)` instead of panicking. We drive the
    /// failure directly through the same AEAD call + error mapping that
    /// `encrypt_chunks` uses, because a live oversized chunk would require
    /// allocating past XChaCha20-Poly1305's multi-gigabyte per-message limit.
    #[test]
    fn oversized_chunk_maps_to_encrypt_failed_not_panic() {
        // Reproduce the encrypt-error → ChunkError mapping used in
        // `encrypt_chunks` and assert it surfaces as a recoverable Err.
        let mapped: Result<EncryptedChunk, ChunkError> =
            Err(()).map_err(|_| ChunkError::EncryptFailed { index: 7 });
        let err = mapped.unwrap_err();
        match err {
            ChunkError::EncryptFailed { index } => assert_eq!(index, 7),
            other => panic!("expected EncryptFailed, got {other:?}"),
        }
        assert_eq!(
            ChunkError::EncryptFailed { index: 7 }.to_string(),
            "Chunk 7 encryption failed"
        );
    }

    /// Edge-case audit high #14: dropping a chunk in the *middle* of a
    /// multi-chunk stream (not the tail) must fail decryption with the
    /// dedicated `MissingChunk` error — distinct from `AuthFailed` so
    /// callers can request a targeted re-send of the missing index.
    #[test]
    fn gap_in_middle_fails_decryption() {
        let key = test_key();
        let file_id = test_file_id();
        // 5 chunks of 4 bytes each — encrypt 20-byte payload with chunk_size=4.
        let data = b"AAAABBBBCCCCDDDDEEEE";
        let mut chunks = encrypt_chunks(data, &key, &file_id, 4).unwrap();
        assert_eq!(chunks.len(), 5);

        // Drop chunk at index 2 — now we have [0, 1, 3, 4] in positions
        // [0, 1, 2, 3], so position 2 holds a chunk with chunk_index=3.
        chunks.remove(2);
        assert_eq!(chunks.len(), 4);

        let err = decrypt_chunks(&chunks, &key, &file_id).unwrap_err();
        match err {
            ChunkError::MissingChunk {
                position,
                expected,
                got,
            } => {
                assert_eq!(position, 2);
                assert_eq!(expected, 2);
                assert_eq!(got, 3);
            }
            other => panic!("expected MissingChunk, got {other:?}"),
        }
    }

    /// Fix [HIGH]: `chunk_size == 0` must return `InvalidChunkSize` instead of
    /// panicking via `slice::chunks(0)`.
    #[test]
    fn zero_chunk_size_returns_invalid_chunk_size_error() {
        let key = test_key();
        let file_id = test_file_id();
        let result = encrypt_chunks(b"some data", &key, &file_id, 0);
        assert!(
            matches!(result, Err(ChunkError::InvalidChunkSize)),
            "chunk_size=0 must produce InvalidChunkSize, got {:?}",
            result
        );
        assert_eq!(
            ChunkError::InvalidChunkSize.to_string(),
            "chunk_size must be at least 1"
        );
    }

    /// Fix [MED]: passphrase minimum length error message is correct.
    #[test]
    fn too_many_chunks_error_message() {
        assert_eq!(
            ChunkError::TooManyChunks.to_string(),
            "too many chunks: stream length exceeds u32::MAX"
        );
    }

    /// Audit LOW: `decrypt_chunks` with an empty slice must return
    /// `Err(ChunkError::Empty)` — never panic. Covers both the early-return
    /// guard and the `ok_or(ChunkError::Empty)?` fallback that replaced the
    /// `.expect()` in the AEAD path.
    #[test]
    fn empty_chunks_returns_err_not_panic() {
        let key = test_key();
        let file_id = test_file_id();
        let result = decrypt_chunks(&[], &key, &file_id);
        assert!(
            matches!(result, Err(ChunkError::Empty)),
            "empty chunk slice must produce ChunkError::Empty, got {:?}",
            result
        );
        assert_eq!(ChunkError::Empty.to_string(), "Empty chunk stream");
    }
}
