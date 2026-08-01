//! Encrypted, content-addressed binary clipboard payloads.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{decrypt, encrypt, CryptoError, ItemKey};

const MAGIC: &[u8; 4] = b"CPB2";
const VERSION: u8 = 1;
/// Keeps one decoded payload bounded while permitting images and files above a
/// single transport frame to be stored locally.
pub const CHUNK_BYTES: usize = 512 * 1024;
const HEADER_BYTES: usize = 4 + 1 + 8 + 4 + 32;

/// Metadata authenticated by the envelope's structure and verified against
/// the recovered plaintext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryMetadata {
    pub byte_len: u64,
    pub chunk_count: u32,
    pub content_hash: String,
}

/// User-facing attributes of an opaque payload.  It never contains a source
/// path: retaining a path would leak a username through IPC, sync and logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub filename: String,
    pub mime_type: String,
}

impl FileMetadata {
    pub fn new(filename: impl Into<String>, mime_type: impl Into<String>) -> Option<Self> {
        let filename = filename.into();
        let mime_type = mime_type.into();
        (filename.len() <= 255
            && !filename.is_empty()
            && std::path::Path::new(&filename)
                .file_name()
                .is_some_and(|name| name == std::ffi::OsStr::new(&filename))
            && mime_type.len() <= 255
            && mime_type.contains('/'))
        .then_some(Self {
            filename,
            mime_type,
        })
    }
}

/// A deterministic logical id for a binary value.  The UUID spelling preserves
/// the existing item-id contract while the full digest remains the dedup key.
#[must_use]
pub fn item_id(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    uuid::Uuid::from_bytes(id).to_string()
}

#[must_use]
pub fn metadata(bytes: &[u8]) -> BinaryMetadata {
    let chunks = bytes.len().div_ceil(CHUNK_BYTES);
    BinaryMetadata {
        byte_len: bytes.len() as u64,
        chunk_count: u32::try_from(chunks).unwrap_or(u32::MAX),
        content_hash: hex::encode(Sha256::digest(bytes)),
    }
}

/// Seal bytes into independent authenticated chunks.  Chunk AAD includes its
/// zero-based index, so replacing or reordering chunks fails closed.
pub fn seal(bytes: &[u8], key: &ItemKey, id: &str) -> Result<Vec<u8>, CryptoError> {
    let meta = metadata(bytes);
    let mut out = Vec::with_capacity(bytes.len().saturating_add(HEADER_BYTES));
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&meta.byte_len.to_be_bytes());
    out.extend_from_slice(&meta.chunk_count.to_be_bytes());
    let digest = Sha256::digest(bytes);
    out.extend_from_slice(&digest);

    for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        let chunk_id = format!("binary:{id}:{index}");
        let (nonce, ciphertext) = encrypt(chunk, key, &chunk_id)?;
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
        out.extend_from_slice(&ciphertext);
    }
    Ok(out)
}

/// Open and verify a binary chunk envelope.
pub fn open(envelope: &[u8], key: &ItemKey, id: &str) -> Result<Vec<u8>, CryptoError> {
    if envelope.len() < HEADER_BYTES || &envelope[..4] != MAGIC || envelope[4] != VERSION {
        return Err(CryptoError::AuthFailed);
    }
    let byte_len = u64::from_be_bytes(
        envelope[5..13]
            .try_into()
            .map_err(|_| CryptoError::AuthFailed)?,
    );
    let chunk_count = u32::from_be_bytes(
        envelope[13..17]
            .try_into()
            .map_err(|_| CryptoError::AuthFailed)?,
    );
    let expected_hash = &envelope[17..49];
    let mut offset = HEADER_BYTES;
    let mut plain =
        Vec::with_capacity(usize::try_from(byte_len).map_err(|_| CryptoError::AuthFailed)?);
    for index in 0..chunk_count as usize {
        let end_nonce = offset.checked_add(24).ok_or(CryptoError::AuthFailed)?;
        let end_len = end_nonce.checked_add(4).ok_or(CryptoError::AuthFailed)?;
        if end_len > envelope.len() {
            return Err(CryptoError::AuthFailed);
        }
        let size = u32::from_be_bytes(
            envelope[end_nonce..end_len]
                .try_into()
                .map_err(|_| CryptoError::AuthFailed)?,
        ) as usize;
        let end_chunk = end_len.checked_add(size).ok_or(CryptoError::AuthFailed)?;
        if end_chunk > envelope.len() {
            return Err(CryptoError::AuthFailed);
        }
        let chunk_id = format!("binary:{id}:{index}");
        let decoded = decrypt(
            &envelope[end_len..end_chunk],
            &envelope[offset..end_nonce],
            key,
            &chunk_id,
        )?;
        if decoded.len() > CHUNK_BYTES {
            return Err(CryptoError::AuthFailed);
        }
        plain.extend_from_slice(&decoded);
        offset = end_chunk;
    }
    if offset != envelope.len()
        || plain.len() as u64 != byte_len
        || Sha256::digest(&plain).as_slice() != expected_hash
    {
        return Err(CryptoError::AuthFailed);
    }
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Keyring;

    #[test]
    fn chunks_round_trip_and_are_content_addressed() {
        let bytes = vec![42; CHUNK_BYTES + 17];
        let key = Keyring::from_secret(&[9; 32]).item_key();
        let id = item_id(&bytes);
        let sealed = seal(&bytes, &key, &id).unwrap();
        assert_eq!(open(&sealed, &key, &id).unwrap(), bytes);
        assert_eq!(metadata(&bytes).chunk_count, 2);
        assert_eq!(id, item_id(&bytes));
    }

    #[test]
    fn swapped_chunk_fails_authentication() {
        let bytes = vec![7; CHUNK_BYTES + 3];
        let key = Keyring::from_secret(&[3; 32]).item_key();
        let id = item_id(&bytes);
        let mut sealed = seal(&bytes, &key, &id).unwrap();
        let first = HEADER_BYTES;
        sealed[first] ^= 1;
        assert!(open(&sealed, &key, &id).is_err());
    }
}
