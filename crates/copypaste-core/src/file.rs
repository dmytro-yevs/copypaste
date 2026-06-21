//! Arbitrary-file chunking and encryption pipeline for clipboard files.
//!
//! Pipeline (encode):
//!   raw file bytes  →  split into 512 KB chunks
//!                   →  encrypt each chunk with XChaCha20-Poly1305
//!
//! Pipeline (decode):
//!   encrypted chunks  →  decrypt  →  reassemble  →  raw file bytes
//!
//! Unlike [`crate::image`], files are NEVER decoded/re-encoded — the raw bytes
//! are chunked verbatim, so the round-trip is byte-identical for any content
//! (`content_type = "file"`). The chunk/blob substrate is content-agnostic, so
//! this module reuses [`crate::image::chunks_to_blob`] /
//! [`crate::image::chunks_from_blob`] rather than duplicating them.

use thiserror::Error;

use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};

/// 512 KB chunk size (mirrors [`crate::image::IMAGE_CHUNK_SIZE`]).
pub const FILE_CHUNK_SIZE: usize = 512 * 1024;
/// Maximum accepted file size (raw bytes): 100 MB.
pub const MAX_FILE_BYTES: usize = 100 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum FileError {
    #[error("File too large: {actual} bytes (max {max})")]
    TooLarge { actual: usize, max: usize },
    #[error("File name must not be empty")]
    EmptyFilename,
    #[error("Chunk encryption error: {0}")]
    Chunk(#[from] ChunkError),
}

/// Metadata stored alongside the encrypted chunk blob (serialized into the
/// `clipboard_items.blob_ref` JSON column).
#[derive(Debug, Clone)]
pub struct FileMeta {
    /// Original file name (as captured from the source clipboard).
    pub filename: String,
    /// MIME type of the file.
    pub mime: String,
    /// Original raw byte count.
    pub original_size: u64,
    /// Number of encrypted chunks.
    pub chunk_count: u32,
    /// 16-byte id used as AAD context for chunk encryption.
    pub file_id: [u8; 16],
}

/// Full encode pipeline:
///   raw file bytes → split into chunks → encrypt
///
/// `max_bytes` is the configured raw-byte ceiling (the daemon threads
/// `AppConfig::max_file_size_bytes` here). Passing `0` falls back to the
/// library default [`MAX_FILE_BYTES`] so callers without config still get a
/// sane bound.
///
/// Rejects an empty `filename` ([`FileError::EmptyFilename`]) and a `raw`
/// payload larger than the resolved cap ([`FileError::TooLarge`]). Unlike the
/// image pipeline there is NO decode/re-encode step: the raw bytes are chunked
/// and encrypted verbatim.
///
/// Returns `(FileMeta, Vec<EncryptedChunk>)`.
pub fn encode_file(
    raw: &[u8],
    filename: &str,
    mime: &str,
    key: &[u8; 32],
    file_id: &[u8; 16],
    max_bytes: usize,
) -> Result<(FileMeta, Vec<EncryptedChunk>), FileError> {
    if filename.is_empty() {
        return Err(FileError::EmptyFilename);
    }

    let max = if max_bytes == 0 {
        MAX_FILE_BYTES
    } else {
        max_bytes
    };
    if raw.len() > max {
        return Err(FileError::TooLarge {
            actual: raw.len(),
            max,
        });
    }

    let original_size = raw.len() as u64;
    let chunks = encrypt_chunks(raw, key, file_id, FILE_CHUNK_SIZE)?;
    // chunks.len() is provably ≤ ceil(max / FILE_CHUNK_SIZE), well within u32.
    // try_from + map keeps the invariant explicit and avoids a silent truncation
    // should the gate ever widen past u32::MAX chunks.
    let chunk_count =
        u32::try_from(chunks.len()).map_err(|_| FileError::Chunk(ChunkError::TooManyChunks))?;

    let meta = FileMeta {
        filename: filename.to_string(),
        mime: mime.to_string(),
        original_size,
        chunk_count,
        file_id: *file_id,
    };

    Ok((meta, chunks))
}

/// Full decode pipeline:
///   encrypted chunks → decrypt → reassemble → raw file bytes
///
/// `file_id` MUST be the same value passed to [`encode_file`]; the chunk AEAD
/// binds it as AAD, so a wrong id fails the integrity check (mirrors
/// [`crate::image::decode_image`]).
///
/// For callers that process the file bytes before use, prefer
/// [`decode_file_zeroizing`] which scrubs the heap on drop.
pub fn decode_file(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<Vec<u8>, FileError> {
    let bytes = decrypt_chunks(chunks, key, file_id)?;
    Ok(bytes)
}

/// Like [`decode_file`] but wraps the plaintext in `Zeroizing<Vec<u8>>` so the
/// decrypted file bytes are scrubbed from the heap when the caller drops the
/// buffer.
///
/// CopyPaste-dgqm: pre-wired so any future expansion of the file-export path
/// can use this variant and automatically inherit the zeroize-on-drop contract,
/// preventing plaintext from lingering in freed memory between decryption and use.
pub fn decode_file_zeroizing(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<zeroize::Zeroizing<Vec<u8>>, FileError> {
    let bytes = decrypt_chunks(chunks, key, file_id)?;
    Ok(zeroize::Zeroizing::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{chunks_from_blob, chunks_to_blob};

    fn test_key() -> [u8; 32] {
        [0x22u8; 32]
    }

    fn test_file_id() -> [u8; 16] {
        [0xDDu8; 16]
    }

    #[test]
    fn round_trip_through_blob() {
        let key = test_key();
        let file_id = test_file_id();
        let raw = b"the quick brown fox jumps over the lazy dog".to_vec();

        let (meta, chunks) = encode_file(&raw, "fox.txt", "text/plain", &key, &file_id, 0).unwrap();
        assert_eq!(meta.filename, "fox.txt");
        assert_eq!(meta.mime, "text/plain");
        assert_eq!(meta.original_size, raw.len() as u64);
        assert_eq!(meta.chunk_count as usize, chunks.len());
        assert_eq!(meta.file_id, file_id);

        // Serialize to the SQLite blob format and back (reused image helpers).
        let blob = chunks_to_blob(&chunks).unwrap();
        let recovered_chunks = chunks_from_blob(&blob).unwrap();

        let recovered = decode_file(&recovered_chunks, &key, &file_id).unwrap();
        assert_eq!(recovered, raw, "file bytes must round-trip verbatim");
    }

    #[test]
    fn multi_chunk_for_large_input() {
        let key = test_key();
        let file_id = test_file_id();
        // Just over one chunk forces a second chunk — and exercises the
        // raw-bytes (no decode) path with a non-trivial payload.
        let raw = vec![0x5Au8; FILE_CHUNK_SIZE + 100];

        let (meta, chunks) = encode_file(
            &raw,
            "blob.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap();
        assert_eq!(chunks.len(), 2, "input > FILE_CHUNK_SIZE must split");
        assert_eq!(meta.chunk_count, 2);

        let recovered = decode_file(&chunks, &key, &file_id).unwrap();
        assert_eq!(recovered, raw);
    }

    #[test]
    fn oversized_file_rejected() {
        let key = test_key();
        let file_id = test_file_id();
        let huge = vec![0u8; MAX_FILE_BYTES + 1];
        let err = encode_file(
            &huge,
            "huge.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, FileError::TooLarge { .. }));
    }

    #[test]
    fn configured_cap_below_default_rejects() {
        let key = test_key();
        let file_id = test_file_id();
        let raw = vec![0u8; 2048];
        // A small configured cap must reject a payload above it.
        let err = encode_file(
            &raw,
            "x.bin",
            "application/octet-stream",
            &key,
            &file_id,
            1024,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            FileError::TooLarge {
                actual: 2048,
                max: 1024
            }
        ));
    }

    #[test]
    fn empty_filename_rejected() {
        let key = test_key();
        let file_id = test_file_id();
        let err = encode_file(b"data", "", "text/plain", &key, &file_id, 0).unwrap_err();
        assert!(matches!(err, FileError::EmptyFilename));
    }

    #[test]
    fn wrong_key_fails_decode() {
        let key = test_key();
        let bad_key = [0xFFu8; 32];
        let file_id = test_file_id();
        let (_, chunks) = encode_file(
            b"secret bytes",
            "s.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap();
        let err = decode_file(&chunks, &bad_key, &file_id).unwrap_err();
        assert!(matches!(err, FileError::Chunk(_)));
    }

    #[test]
    fn wrong_file_id_fails_decode() {
        let key = test_key();
        let file_id = test_file_id();
        let bad_id = [0x00u8; 16];
        let (_, chunks) = encode_file(
            b"secret bytes",
            "s.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap();
        let err = decode_file(&chunks, &key, &bad_id).unwrap_err();
        assert!(matches!(err, FileError::Chunk(_)));
    }

    #[test]
    fn empty_file_round_trips() {
        // An empty (0-byte) file still has a valid name; encrypt_chunks emits a
        // single empty final chunk, so the round-trip must reproduce no bytes.
        let key = test_key();
        let file_id = test_file_id();
        let (meta, chunks) =
            encode_file(&[], "empty.txt", "text/plain", &key, &file_id, 0).unwrap();
        assert_eq!(meta.original_size, 0);
        let recovered = decode_file(&chunks, &key, &file_id).unwrap();
        assert!(recovered.is_empty());
    }

    // CopyPaste-dgqm: Zeroizing export path tests.

    #[test]
    fn decode_file_zeroizing_matches_decode_file() {
        let key = test_key();
        let file_id = test_file_id();
        let raw = b"secret file data that must be zeroized on drop".to_vec();

        let (_, chunks) = encode_file(
            &raw,
            "sec.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap();

        let plain = decode_file(&chunks, &key, &file_id).unwrap();
        let zeroizing = decode_file_zeroizing(&chunks, &key, &file_id).unwrap();

        // The Zeroizing wrapper must be transparent: same bytes, different lifetime guarantee.
        assert_eq!(
            *zeroizing, plain,
            "decode_file_zeroizing must produce identical bytes to decode_file"
        );
    }

    #[test]
    fn decode_file_zeroizing_wrong_key_fails() {
        let key = test_key();
        let bad_key = [0xEEu8; 32];
        let file_id = test_file_id();
        let (_, chunks) = encode_file(
            b"data",
            "d.bin",
            "application/octet-stream",
            &key,
            &file_id,
            0,
        )
        .unwrap();

        let err = decode_file_zeroizing(&chunks, &bad_key, &file_id).unwrap_err();
        assert!(
            matches!(err, FileError::Chunk(_)),
            "wrong key must fail AEAD auth on the Zeroizing path: {err:?}"
        );
    }
}
