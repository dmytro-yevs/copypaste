//! Encrypted, content-addressed binary clipboard payloads.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::{NONCE_LEN, TAG_LEN};
use crate::{decrypt, encrypt, CryptoError, ItemKey};

const MAGIC: &[u8; 4] = b"CPB2";
const VERSION: u8 = 2;
/// Keeps one decoded payload bounded while permitting images and files above a
/// single transport frame to be stored locally.
pub const CHUNK_BYTES: usize = 512 * 1024;
const HEADER_BYTES: usize = 4 + 1 + 8 + 4 + 32;
/// Binary capture is bounded by the same ceiling that storage and every sync
/// transport enforce.  This header is untrusted until every chunk authenticates,
/// so it must not be able to request an arbitrary allocation.
const MAX_BINARY_BYTES: u64 = copypaste_ipc::MAX_CONTENT_BYTES as u64;

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

    /// Parse metadata received from an authenticated transport. `Deserialize`
    /// alone is not sufficient: it bypasses the constructor's basename rule.
    #[must_use]
    pub fn from_json(value: &str) -> Option<Self> {
        serde_json::from_str::<Self>(value)
            .ok()
            .filter(|metadata| metadata.is_valid())
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        Self::new(self.filename.clone(), self.mime_type.clone()).is_some()
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
    let chunks = bytes.len().div_ceil(CHUNK_BYTES).max(1);
    BinaryMetadata {
        byte_len: bytes.len() as u64,
        chunk_count: u32::try_from(chunks).unwrap_or(u32::MAX),
        content_hash: hex::encode(Sha256::digest(bytes)),
    }
}

fn header(bytes: &[u8], meta: &BinaryMetadata) -> [u8; HEADER_BYTES] {
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(MAGIC);
    header[4] = VERSION;
    header[5..13].copy_from_slice(&meta.byte_len.to_be_bytes());
    header[13..17].copy_from_slice(&meta.chunk_count.to_be_bytes());
    header[17..].copy_from_slice(&Sha256::digest(bytes));
    header
}

/// RustCrypto STREAM requires raw key access, which `ItemKey` deliberately
/// withholds here. Bind its total, position and final-block properties through
/// the public item-AEAD AAD instead of widening the key boundary.
fn chunk_aad_prefix(id: &str, header: &[u8; HEADER_BYTES]) -> String {
    format!("binary:v2|{}:{}|{}", id.len(), id, hex::encode(header))
}

fn chunk_aad(prefix: &str, index: u32, is_final: bool) -> String {
    format!("{prefix}|{index}|{}", u8::from(is_final))
}

/// Seal bytes into header-, position- and final-block-authenticated chunks.
pub fn seal(bytes: &[u8], key: &ItemKey, id: &str) -> Result<Vec<u8>, CryptoError> {
    if bytes.len() as u64 > MAX_BINARY_BYTES {
        return Err(CryptoError::AuthFailed);
    }
    let meta = metadata(bytes);
    let header = header(bytes, &meta);
    let aad_prefix = chunk_aad_prefix(id, &header);
    let mut out = Vec::with_capacity(bytes.len().saturating_add(HEADER_BYTES));
    out.extend_from_slice(&header);

    for index in 0..meta.chunk_count {
        let start = (index as usize)
            .checked_mul(CHUNK_BYTES)
            .ok_or(CryptoError::Internal("binary chunk offset overflow"))?;
        let end = start.saturating_add(CHUNK_BYTES).min(bytes.len());
        let chunk = bytes
            .get(start..end)
            .ok_or(CryptoError::Internal("binary chunk range is invalid"))?;
        let is_final = index + 1 == meta.chunk_count;
        let aad = chunk_aad(&aad_prefix, index, is_final);
        let (nonce, ciphertext) = encrypt(chunk, key, &aad)?;
        let ciphertext_len = u32::try_from(ciphertext.len())
            .map_err(|_| CryptoError::Internal("binary chunk ciphertext is too large"))?;
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext_len.to_be_bytes());
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
    if byte_len > MAX_BINARY_BYTES {
        return Err(CryptoError::AuthFailed);
    }
    let expected_chunks = byte_len.div_ceil(CHUNK_BYTES as u64);
    if expected_chunks.max(1) != u64::from(chunk_count) {
        return Err(CryptoError::AuthFailed);
    }
    let expected_hash = &envelope[17..49];
    let header: &[u8; HEADER_BYTES] = envelope[..HEADER_BYTES]
        .try_into()
        .map_err(|_| CryptoError::AuthFailed)?;
    let aad_prefix = chunk_aad_prefix(id, header);
    let mut offset = HEADER_BYTES;
    // Do not reserve from the peer-controlled `byte_len`.  Every append below
    // follows successful AEAD verification of one structurally-bounded chunk.
    let mut plain = Vec::new();
    for index in 0..chunk_count as usize {
        let end_nonce = offset
            .checked_add(NONCE_LEN)
            .ok_or(CryptoError::AuthFailed)?;
        let end_len = end_nonce.checked_add(4).ok_or(CryptoError::AuthFailed)?;
        if end_len > envelope.len() {
            return Err(CryptoError::AuthFailed);
        }
        let size = u32::from_be_bytes(
            envelope[end_nonce..end_len]
                .try_into()
                .map_err(|_| CryptoError::AuthFailed)?,
        ) as usize;
        let remaining = byte_len
            .checked_sub((index as u64) * CHUNK_BYTES as u64)
            .ok_or(CryptoError::AuthFailed)?;
        let chunk_plain_len = remaining.min(CHUNK_BYTES as u64) as usize;
        if size != chunk_plain_len.saturating_add(TAG_LEN) {
            return Err(CryptoError::AuthFailed);
        }
        let end_chunk = end_len.checked_add(size).ok_or(CryptoError::AuthFailed)?;
        if end_chunk > envelope.len() {
            return Err(CryptoError::AuthFailed);
        }
        let is_final = index + 1 == chunk_count as usize;
        let aad = chunk_aad(&aad_prefix, index as u32, is_final);
        let decoded = decrypt(
            &envelope[end_len..end_chunk],
            &envelope[offset..end_nonce],
            key,
            &aad,
        )?;
        if decoded.len() != chunk_plain_len {
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
    use std::ops::Range;

    fn assert_auth_failed(result: Result<Vec<u8>, CryptoError>) {
        assert!(matches!(result, Err(CryptoError::AuthFailed)));
    }

    fn chunk_records(envelope: &[u8]) -> Vec<Range<usize>> {
        let count = u32::from_be_bytes(envelope[13..17].try_into().unwrap());
        let mut records = Vec::with_capacity(count as usize);
        let mut offset = HEADER_BYTES;
        for _ in 0..count {
            let len_offset = offset + NONCE_LEN;
            let size = u32::from_be_bytes(envelope[len_offset..len_offset + 4].try_into().unwrap())
                as usize;
            let end = len_offset + 4 + size;
            records.push(offset..end);
            offset = end;
        }
        assert_eq!(offset, envelope.len());
        records
    }

    fn select_records(envelope: &[u8], selected: &[usize]) -> Vec<u8> {
        let records = chunk_records(envelope);
        let mut selected_envelope = envelope[..HEADER_BYTES].to_vec();
        for &index in selected {
            selected_envelope.extend_from_slice(&envelope[records[index].clone()]);
        }
        selected_envelope
    }

    fn three_chunk_payload() -> Vec<u8> {
        let mut bytes = vec![1; CHUNK_BYTES];
        bytes.extend(vec![2; CHUNK_BYTES]);
        bytes.extend(vec![3; CHUNK_BYTES]);
        bytes
    }

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
    fn sealing_cannot_exceed_the_envelope_opening_bound() {
        let bytes = vec![0; MAX_BINARY_BYTES as usize + 1];
        let key = Keyring::from_secret(&[9; 32]).item_key();
        let id = item_id(&bytes);
        assert!(matches!(
            seal(&bytes, &key, &id),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn empty_and_exact_boundary_payloads_round_trip() {
        let key = Keyring::from_secret(&[4; 32]).item_key();
        for (size, expected_chunks) in [(0, 1), (CHUNK_BYTES, 1), (2 * CHUNK_BYTES, 2)] {
            let bytes = vec![0x5a; size];
            let id = item_id(&bytes);
            let sealed = seal(&bytes, &key, &id).unwrap();

            assert_eq!(metadata(&bytes).chunk_count, expected_chunks);
            assert_eq!(open(&sealed, &key, &id).unwrap(), bytes);
        }
    }

    #[test]
    fn known_plaintext_prefix_cannot_be_forged_as_a_complete_payload() {
        let known_prefix = vec![0x41; CHUNK_BYTES];
        let mut bytes = known_prefix.clone();
        bytes.extend_from_slice(b"unknown authenticated suffix");
        let key = Keyring::from_secret(&[5; 32]).item_key();
        let id = "chosen-stream";
        let sealed = seal(&bytes, &key, id).unwrap();
        let first = chunk_records(&sealed)[0].clone();
        let mut forged = sealed[..first.end].to_vec();
        let prefix_meta = metadata(&known_prefix);
        forged[..HEADER_BYTES].copy_from_slice(&header(&known_prefix, &prefix_meta));

        assert_auth_failed(open(&forged, &key, id));
    }

    #[test]
    fn missing_chunk_fails_authentication() {
        let bytes = three_chunk_payload();
        let key = Keyring::from_secret(&[6; 32]).item_key();
        let sealed = seal(&bytes, &key, "missing").unwrap();

        assert_auth_failed(open(&select_records(&sealed, &[0, 2]), &key, "missing"));
    }

    #[test]
    fn duplicated_chunk_fails_authentication() {
        let bytes = three_chunk_payload();
        let key = Keyring::from_secret(&[7; 32]).item_key();
        let sealed = seal(&bytes, &key, "duplicated").unwrap();

        assert_auth_failed(open(
            &select_records(&sealed, &[0, 0, 2]),
            &key,
            "duplicated",
        ));
    }

    #[test]
    fn reordered_chunks_fail_authentication() {
        let bytes = three_chunk_payload();
        let key = Keyring::from_secret(&[8; 32]).item_key();
        let sealed = seal(&bytes, &key, "reordered").unwrap();

        assert_auth_failed(open(
            &select_records(&sealed, &[1, 0, 2]),
            &key,
            "reordered",
        ));
    }

    #[test]
    fn cross_stream_chunk_injection_fails_authentication() {
        let key = Keyring::from_secret(&[9; 32]).item_key();
        let source = seal(&vec![0x11; CHUNK_BYTES + 19], &key, "source").unwrap();
        let target = seal(&vec![0x22; CHUNK_BYTES + 19], &key, "target").unwrap();
        let source_records = chunk_records(&source);
        let target_records = chunk_records(&target);
        let mut injected = target.clone();
        injected[target_records[1].clone()].copy_from_slice(&source[source_records[1].clone()]);

        assert_auth_failed(open(&injected, &key, "target"));
    }

    #[test]
    fn every_header_byte_is_tamper_evident() {
        let bytes = vec![0x33; CHUNK_BYTES + 7];
        let key = Keyring::from_secret(&[10; 32]).item_key();
        let sealed = seal(&bytes, &key, "header").unwrap();

        for offset in 0..HEADER_BYTES {
            let mut tampered = sealed.clone();
            tampered[offset] ^= 1;
            assert_auth_failed(open(&tampered, &key, "header"));
        }
    }

    #[test]
    fn wrong_key_and_wrong_aad_fail_authentication() {
        let bytes = b"bound binary";
        let key = Keyring::from_secret(&[11; 32]).item_key();
        let wrong_key = Keyring::from_secret(&[12; 32]).item_key();
        let sealed = seal(bytes, &key, "right-id").unwrap();

        assert_auth_failed(open(&sealed, &wrong_key, "right-id"));
        assert_auth_failed(open(&sealed, &key, "wrong-id"));
    }

    #[test]
    fn tampered_chunk_fails_authentication() {
        let bytes = vec![7; CHUNK_BYTES + 3];
        let key = Keyring::from_secret(&[3; 32]).item_key();
        let id = item_id(&bytes);
        let mut sealed = seal(&bytes, &key, &id).unwrap();
        let first = HEADER_BYTES;
        sealed[first] ^= 1;
        assert_auth_failed(open(&sealed, &key, &id));
    }

    #[test]
    fn transport_metadata_rejects_a_path_even_after_deserializing() {
        assert!(FileMetadata::from_json(
            r#"{"filename":"../private.txt","mime_type":"text/plain"}"#
        )
        .is_none());
    }

    #[test]
    fn an_absurd_untrusted_byte_length_fails_without_allocating() {
        let key = Keyring::from_secret(&[8; 32]).item_key();
        let mut envelope = Vec::from(MAGIC.as_slice());
        envelope.push(VERSION);
        envelope.extend_from_slice(&u64::MAX.to_be_bytes());
        envelope.extend_from_slice(&u32::MAX.to_be_bytes());
        envelope.extend_from_slice(&[0; 32]);

        assert!(matches!(
            open(&envelope, &key, "binary-id"),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn a_header_with_an_impossible_chunk_count_fails_closed() {
        let key = Keyring::from_secret(&[8; 32]).item_key();
        let mut envelope = Vec::from(MAGIC.as_slice());
        envelope.push(VERSION);
        envelope.extend_from_slice(&1u64.to_be_bytes());
        envelope.extend_from_slice(&0u32.to_be_bytes());
        envelope.extend_from_slice(&[0; 32]);

        assert!(matches!(
            open(&envelope, &key, "binary-id"),
            Err(CryptoError::AuthFailed)
        ));
    }
}
