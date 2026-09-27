//! Encrypted, content-addressed binary clipboard payloads.

use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::aead::Buffer;
use image::{GenericImageView, ImageFormat, ImageReader, Limits};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::crypto::{stream_decryptor, stream_encryptor, STREAM_NONCE_LEN, TAG_LEN};
use crate::{CryptoError, ItemKey};

const MAGIC: &[u8; 4] = b"CPB2";
const VERSION: u8 = 3;
/// Keeps one decoded payload bounded while permitting images and files above a
/// single transport frame to be stored locally.
pub const CHUNK_BYTES: usize = 512 * 1024;
const STREAM_NONCE_OFFSET: usize = 4 + 1 + 8 + 32;
const HEADER_BYTES: usize = STREAM_NONCE_OFFSET + STREAM_NONCE_LEN;
const AAD_PREFIX: &[u8] = b"copypaste/v2/binary-stream|";
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
#[serde(deny_unknown_fields)]
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

/// Strict, optional application-identity icon carried in signed metadata.
/// The largest padded standard-base64 spelling of a bounded PNG. This is
/// checked before decoding so an authenticated peer still cannot make us
/// allocate an arbitrary icon buffer.
const MAX_SOURCE_APP_ICON_BASE64_BYTES: usize =
    copypaste_ipc::MAX_SOURCE_APP_ICON_BYTES.div_ceil(3) * 4;
/// The 128px RGBA output needs 64 KiB. Leave bounded headroom for decoder
/// state so a valid maximum-sized PNG does not fail solely on scratch space.
const MAX_SOURCE_APP_ICON_DECODED_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAppIconMetadata {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

impl SourceAppIconMetadata {
    pub fn new(png: &[u8], width: u32, height: u32) -> Option<Self> {
        valid_icon_dimensions(png, width, height).then(|| Self {
            png_base64: STANDARD.encode(png),
            width,
            height,
        })
    }

    pub fn png(&self) -> Option<Vec<u8>> {
        if self.png_base64.len() > MAX_SOURCE_APP_ICON_BASE64_BYTES {
            return None;
        }
        let png = STANDARD.decode(&self.png_base64).ok()?;
        valid_icon_dimensions(&png, self.width, self.height).then_some(png)
    }
}

fn valid_icon_dimensions(png: &[u8], width: u32, height: u32) -> bool {
    if png.is_empty()
        || png.len() > copypaste_ipc::MAX_SOURCE_APP_ICON_BYTES
        || !(1..=copypaste_ipc::SOURCE_APP_ICON_EDGE).contains(&width)
        || !(1..=copypaste_ipc::SOURCE_APP_ICON_EDGE).contains(&height)
    {
        return false;
    }

    let mut reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(copypaste_ipc::SOURCE_APP_ICON_EDGE);
    limits.max_image_height = Some(copypaste_ipc::SOURCE_APP_ICON_EDGE);
    limits.max_alloc = Some(MAX_SOURCE_APP_ICON_DECODED_BYTES);
    reader.limits(limits);
    reader
        .decode()
        .ok()
        .is_some_and(|image| image.dimensions() == (width, height))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PayloadMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_icon: Option<SourceAppIconMetadata>,
}

impl PayloadMetadata {
    #[must_use]
    pub fn new(
        file: Option<FileMetadata>,
        source_app_icon: Option<SourceAppIconMetadata>,
    ) -> Option<Self> {
        let metadata = Self {
            file,
            source_app_icon,
        };
        metadata.has_valid_components().then_some(metadata)
    }

    #[must_use]
    pub fn is_valid_for(&self, content_type: &str) -> bool {
        self.has_valid_components()
            && if content_type == copypaste_ipc::content_type::FILE {
                self.file.is_some()
            } else {
                self.file.is_none() && self.source_app_icon.is_some()
            }
    }

    fn has_valid_components(&self) -> bool {
        let file_is_valid = self.file.as_ref().is_none_or(FileMetadata::is_valid);
        let icon_is_valid = self
            .source_app_icon
            .as_ref()
            .is_none_or(|icon| icon.png().is_some());
        file_is_valid && icon_is_valid && (self.file.is_some() || self.source_app_icon.is_some())
    }

    #[must_use]
    pub fn to_json(&self, content_type: &str) -> Option<String> {
        self.is_valid_for(content_type)
            .then(|| serde_json::to_string(self).ok())
            .flatten()
            .filter(|json| json.len() <= copypaste_ipc::MAX_SYNC_METADATA_BYTES)
    }

    pub fn from_json(value: &str, content_type: &str) -> Option<Self> {
        if value.len() > copypaste_ipc::MAX_SYNC_METADATA_BYTES {
            return None;
        }
        if let Some(file) = FileMetadata::from_json(value) {
            return (content_type == copypaste_ipc::content_type::FILE).then_some(Self {
                file: Some(file),
                source_app_icon: None,
            });
        }
        let metadata: Self = serde_json::from_str(value).ok()?;
        metadata.is_valid_for(content_type).then_some(metadata)
    }
}

/// SHA-256 of the payload — the one identity every other spelling is derived
/// from: the item id, the envelope header the STREAM AAD covers, and the row's
/// `content_hash`. Computing it once and threading it through is what keeps a
/// 4 MiB capture from being hashed four times over.
#[must_use]
pub fn content_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// The same string [`crate::storage::compute_content_hash`] produces for the
/// same bytes, from a digest already in hand. Pinned by
/// `the_content_hash_of_a_digest_is_the_stores_own_value`.
#[must_use]
pub fn content_hash(digest: &[u8; 32]) -> String {
    hex::encode(digest)
}

/// A deterministic logical id for a binary value.  The UUID spelling preserves
/// the existing item-id contract while the full digest remains the dedup key.
#[must_use]
pub fn item_id(bytes: &[u8]) -> String {
    item_id_from_digest(&content_digest(bytes))
}

#[must_use]
pub fn item_id_from_digest(digest: &[u8; 32]) -> String {
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    uuid::Uuid::from_bytes(id).to_string()
}

fn chunk_count(byte_len: usize) -> u32 {
    u32::try_from(byte_len.div_ceil(CHUNK_BYTES).max(1)).unwrap_or(u32::MAX)
}

#[must_use]
pub fn metadata(bytes: &[u8]) -> BinaryMetadata {
    BinaryMetadata {
        byte_len: bytes.len() as u64,
        chunk_count: chunk_count(bytes.len()),
        content_hash: content_hash(&content_digest(bytes)),
    }
}

fn header(
    digest: &[u8; 32],
    byte_len: u64,
    stream_nonce: &[u8; STREAM_NONCE_LEN],
) -> [u8; HEADER_BYTES] {
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(MAGIC);
    header[4] = VERSION;
    header[5..13].copy_from_slice(&byte_len.to_be_bytes());
    header[13..STREAM_NONCE_OFFSET].copy_from_slice(digest);
    header[STREAM_NONCE_OFFSET..].copy_from_slice(stream_nonce);
    header
}

/// The tail of `out` from `start` onwards, as an AEAD buffer.
///
/// STREAM's in-place API appends the tag to the buffer it is handed and
/// truncates it away again on the way back, so pointing it at the end of the
/// output vector removes the per-chunk allocate-then-copy-twice without moving
/// a byte of the framing.
struct Tail<'a> {
    out: &'a mut Zeroizing<Vec<u8>>,
    start: usize,
}

impl AsRef<[u8]> for Tail<'_> {
    fn as_ref(&self) -> &[u8] {
        &self.out[self.start..]
    }
}

impl AsMut<[u8]> for Tail<'_> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.out[self.start..]
    }
}

impl Buffer for Tail<'_> {
    fn extend_from_slice(&mut self, other: &[u8]) -> chacha20poly1305::aead::Result<()> {
        self.out.extend_from_slice(other);
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        self.out.truncate(self.start + len);
    }
}

fn stream_aad(id: &str, header: &[u8; HEADER_BYTES]) -> Vec<u8> {
    let id = id.as_bytes();
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + 24 + id.len() + header.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(id.len().to_string().as_bytes());
    aad.push(b':');
    aad.extend_from_slice(id);
    aad.extend_from_slice(header);
    aad
}

/// Seal bytes with RustCrypto STREAM, binding the item id and envelope header.
pub fn seal(bytes: &[u8], key: &ItemKey, id: &str) -> Result<Vec<u8>, CryptoError> {
    seal_with_digest(bytes, &content_digest(bytes), key, id)
}

/// [`seal`] for a caller that already holds [`content_digest`] of these exact
/// bytes. The digest goes into the header verbatim, so it must remain
/// `SHA-256(bytes)`: it is covered by the STREAM AAD and [`open`] verifies the
/// recovered plaintext against it.
pub fn seal_with_digest(
    bytes: &[u8],
    digest: &[u8; 32],
    key: &ItemKey,
    id: &str,
) -> Result<Vec<u8>, CryptoError> {
    if bytes.len() as u64 > MAX_BINARY_BYTES {
        return Err(CryptoError::AuthFailed);
    }
    let chunk_count = chunk_count(bytes.len());
    let mut stream_nonce = [0u8; STREAM_NONCE_LEN];
    OsRng.fill_bytes(&mut stream_nonce);
    let header = header(digest, bytes.len() as u64, &stream_nonce);
    let aad = stream_aad(id, &header);
    let tag_bytes = (chunk_count as usize).saturating_mul(TAG_LEN);
    let mut out = Zeroizing::new(Vec::with_capacity(
        bytes
            .len()
            .saturating_add(HEADER_BYTES)
            .saturating_add(tag_bytes),
    ));
    out.extend_from_slice(&header);

    let mut stream = stream_encryptor(key, &stream_nonce);
    let final_index = chunk_count as usize - 1;
    for index in 0..final_index {
        let start = index
            .checked_mul(CHUNK_BYTES)
            .ok_or(CryptoError::Internal("binary chunk offset overflow"))?;
        let end = start
            .checked_add(CHUNK_BYTES)
            .ok_or(CryptoError::Internal("binary chunk offset overflow"))?;
        let chunk = bytes
            .get(start..end)
            .ok_or(CryptoError::Internal("binary chunk range is invalid"))?;
        let record = out.len();
        out.extend_from_slice(chunk);
        stream
            .encrypt_next_in_place(
                &aad,
                &mut Tail {
                    out: &mut out,
                    start: record,
                },
            )
            .map_err(|_| CryptoError::Internal("STREAM rejected the binary chunk"))?;
    }

    let final_start = final_index
        .checked_mul(CHUNK_BYTES)
        .ok_or(CryptoError::Internal("binary chunk offset overflow"))?;
    let final_chunk = bytes
        .get(final_start..)
        .ok_or(CryptoError::Internal("binary chunk range is invalid"))?;
    let record = out.len();
    out.extend_from_slice(final_chunk);
    stream
        .encrypt_last_in_place(
            &aad,
            &mut Tail {
                out: &mut out,
                start: record,
            },
        )
        .map_err(|_| CryptoError::Internal("STREAM rejected the binary chunk"))?;
    Ok(std::mem::take(&mut *out))
}

/// Open and verify a binary chunk envelope.
pub fn open(envelope: &[u8], key: &ItemKey, id: &str) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if envelope.len() < HEADER_BYTES || &envelope[..4] != MAGIC || envelope[4] != VERSION {
        return Err(CryptoError::AuthFailed);
    }
    let byte_len = u64::from_be_bytes(
        envelope[5..13]
            .try_into()
            .map_err(|_| CryptoError::AuthFailed)?,
    );
    if byte_len > MAX_BINARY_BYTES {
        return Err(CryptoError::AuthFailed);
    }
    let chunk_count = usize::try_from(byte_len.div_ceil(CHUNK_BYTES as u64).max(1))
        .map_err(|_| CryptoError::AuthFailed)?;
    let plain_len = usize::try_from(byte_len).map_err(|_| CryptoError::AuthFailed)?;
    let expected_envelope_len = plain_len
        .checked_add(HEADER_BYTES)
        .and_then(|len| len.checked_add(chunk_count.checked_mul(TAG_LEN)?))
        .ok_or(CryptoError::AuthFailed)?;
    if envelope.len() != expected_envelope_len {
        return Err(CryptoError::AuthFailed);
    }
    let expected_hash = &envelope[13..STREAM_NONCE_OFFSET];
    let header: &[u8; HEADER_BYTES] = envelope[..HEADER_BYTES]
        .try_into()
        .map_err(|_| CryptoError::AuthFailed)?;
    let stream_nonce: &[u8; STREAM_NONCE_LEN] = envelope[STREAM_NONCE_OFFSET..HEADER_BYTES]
        .try_into()
        .map_err(|_| CryptoError::AuthFailed)?;
    let aad = stream_aad(id, header);
    let mut stream = stream_decryptor(key, stream_nonce);
    let mut offset = HEADER_BYTES;
    // Only now that `expected_envelope_len` has been matched is `plain_len` a
    // length this envelope actually carries; presizing any earlier would let an
    // untrusted header ask for an arbitrary allocation
    // (`an_absurd_untrusted_byte_length_fails_without_allocating`). The tag
    // slack is the room STREAM's in-place decrypt needs before it truncates.
    // Hold recovered chunks in Zeroizing for the whole open: a mid-stream
    // AuthFailed must not leave decrypted plaintext on the heap.
    let mut plain = Zeroizing::new(Vec::with_capacity(plain_len.saturating_add(TAG_LEN)));
    for _ in 0..chunk_count - 1 {
        let end = offset
            .checked_add(CHUNK_BYTES + TAG_LEN)
            .ok_or(CryptoError::AuthFailed)?;
        let ciphertext = envelope.get(offset..end).ok_or(CryptoError::AuthFailed)?;
        let record = plain.len();
        plain.extend_from_slice(ciphertext);
        stream
            .decrypt_next_in_place(
                &aad,
                &mut Tail {
                    out: &mut plain,
                    start: record,
                },
            )
            .map_err(|_| CryptoError::AuthFailed)?;
        if plain.len() - record != CHUNK_BYTES {
            return Err(CryptoError::AuthFailed);
        }
        offset = end;
    }

    let final_plain_len = plain_len
        .checked_sub((chunk_count - 1).saturating_mul(CHUNK_BYTES))
        .ok_or(CryptoError::AuthFailed)?;
    let final_ciphertext_len = final_plain_len
        .checked_add(TAG_LEN)
        .ok_or(CryptoError::AuthFailed)?;
    let end = offset
        .checked_add(final_ciphertext_len)
        .ok_or(CryptoError::AuthFailed)?;
    let ciphertext = envelope.get(offset..end).ok_or(CryptoError::AuthFailed)?;
    let record = plain.len();
    plain.extend_from_slice(ciphertext);
    stream
        .decrypt_last_in_place(
            &aad,
            &mut Tail {
                out: &mut plain,
                start: record,
            },
        )
        .map_err(|_| CryptoError::AuthFailed)?;
    if plain.len() - record != final_plain_len {
        return Err(CryptoError::AuthFailed);
    }
    offset = end;
    let actual_hash = content_digest(&plain);
    if offset != envelope.len()
        || plain.len() as u64 != byte_len
        || actual_hash.as_slice() != expected_hash
    {
        return Err(CryptoError::AuthFailed);
    }
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }
    use crate::Keyring;
    use std::ops::Range;

    fn assert_auth_failed(result: Result<Zeroizing<Vec<u8>>, CryptoError>) {
        assert!(matches!(result, Err(CryptoError::AuthFailed)));
    }

    fn chunk_records(envelope: &[u8]) -> Vec<Range<usize>> {
        let byte_len = u64::from_be_bytes(envelope[5..13].try_into().unwrap()) as usize;
        let count = byte_len.div_ceil(CHUNK_BYTES).max(1);
        let mut records = Vec::with_capacity(count);
        let mut offset = HEADER_BYTES;
        for index in 0..count {
            let plaintext_len = if index + 1 == count {
                byte_len - index * CHUNK_BYTES
            } else {
                CHUNK_BYTES
            };
            let end = offset + plaintext_len + TAG_LEN;
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
        assert_eq!(open(&sealed, &key, &id).unwrap().as_slice(), bytes);
        assert_eq!(metadata(&bytes).chunk_count, 2);
        assert_eq!(id, item_id(&bytes));
    }

    #[test]
    fn open_plaintext_is_zeroizing() {
        fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>(_: &T) {}
        let bytes = b"binary secret";
        let key = Keyring::from_secret(&[9; 32]).item_key();
        let id = item_id(bytes);
        let sealed = seal(bytes, &key, &id).unwrap();
        let out = open(&sealed, &key, &id).unwrap();
        assert_zeroize_on_drop(&out);
        assert_eq!(out.as_slice(), bytes);
    }

    /// The digest is threaded through instead of recomputed at each spelling,
    /// so every spelling must still be the one it was: the id, the row's
    /// `content_hash` and the header the STREAM AAD covers.
    #[test]
    fn the_content_hash_of_a_digest_is_the_stores_own_value() {
        for bytes in [b"".as_slice(), b"a payload", &vec![0x7f; CHUNK_BYTES + 5]] {
            let digest = content_digest(bytes);
            assert_eq!(
                content_hash(&digest),
                crate::storage::compute_content_hash(bytes)
            );
            assert_eq!(item_id_from_digest(&digest), item_id(bytes));
            assert_eq!(metadata(bytes).content_hash, content_hash(&digest));
        }
    }

    /// `seal_with_digest` must produce an envelope indistinguishable from
    /// `seal`'s, header included — a digest of anything but the plaintext would
    /// be a header `open` refuses.
    #[test]
    fn sealing_with_a_precomputed_digest_writes_the_same_header() {
        let bytes = vec![0x6c; CHUNK_BYTES + 11];
        let key = Keyring::from_secret(&[15; 32]).item_key();
        let id = item_id(&bytes);
        let threaded = seal_with_digest(&bytes, &content_digest(&bytes), &key, &id).unwrap();
        let hashed = seal(&bytes, &key, &id).unwrap();

        assert_eq!(
            threaded[..STREAM_NONCE_OFFSET],
            hashed[..STREAM_NONCE_OFFSET]
        );
        assert_eq!(threaded.len(), hashed.len());
        assert_eq!(open(&threaded, &key, &id).unwrap().as_slice(), bytes);
    }

    #[test]
    fn maximum_payload_round_trips_and_one_extra_byte_is_rejected() {
        let key = Keyring::from_secret(&[9; 32]).item_key();
        let maximum = vec![0; MAX_BINARY_BYTES as usize];
        let sealed = seal(&maximum, &key, "maximum").unwrap();
        assert_eq!(open(&sealed, &key, "maximum").unwrap().as_slice(), maximum);

        let bytes = vec![0; MAX_BINARY_BYTES as usize + 1];
        assert!(matches!(
            seal(&bytes, &key, "too-large"),
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
            assert_eq!(open(&sealed, &key, &id).unwrap().as_slice(), bytes);
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
        let nonce = sealed[STREAM_NONCE_OFFSET..HEADER_BYTES]
            .try_into()
            .unwrap();
        forged[..HEADER_BYTES].copy_from_slice(&header(
            &content_digest(&known_prefix),
            known_prefix.len() as u64,
            nonce,
        ));

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
    fn truncation_and_trailing_bytes_fail_authentication() {
        let bytes = vec![0x44; CHUNK_BYTES + 23];
        let key = Keyring::from_secret(&[13; 32]).item_key();
        let sealed = seal(&bytes, &key, "bounded").unwrap();

        assert_auth_failed(open(&sealed[..sealed.len() - 1], &key, "bounded"));
        let mut trailing = sealed;
        trailing.push(0);
        assert_auth_failed(open(&trailing, &key, "bounded"));
    }

    #[test]
    fn stream_envelope_has_only_header_ciphertext_and_tags() {
        let bytes = vec![0x21; CHUNK_BYTES + 3];
        let key = Keyring::from_secret(&[14; 32]).item_key();
        let sealed = seal(&bytes, &key, "minimal").unwrap();
        let resealed = seal(&bytes, &key, "minimal").unwrap();

        assert_eq!(&sealed[..4], MAGIC);
        assert_eq!(sealed[4], VERSION);
        assert_eq!(HEADER_BYTES, 64);
        assert_eq!(sealed.len(), HEADER_BYTES + bytes.len() + 2 * TAG_LEN);
        assert_ne!(
            &sealed[STREAM_NONCE_OFFSET..HEADER_BYTES],
            &resealed[STREAM_NONCE_OFFSET..HEADER_BYTES]
        );
    }

    #[test]
    fn transport_metadata_rejects_a_path_even_after_deserializing() {
        assert!(FileMetadata::from_json(
            r#"{"filename":"../private.txt","mime_type":"text/plain"}"#
        )
        .is_none());
    }

    #[test]
    fn source_icon_envelope_is_strict_and_preserves_legacy_file_metadata() {
        let icon = SourceAppIconMetadata::new(&png(64, 128), 64, 128).expect("valid icon");
        let file = FileMetadata::new("report.pdf", "application/pdf").unwrap();
        let metadata = PayloadMetadata {
            file: Some(file.clone()),
            source_app_icon: Some(icon.clone()),
        };
        let json = metadata.to_json(copypaste_ipc::content_type::FILE).unwrap();
        assert_eq!(
            PayloadMetadata::from_json(&json, copypaste_ipc::content_type::FILE),
            Some(metadata)
        );

        let icon_only = PayloadMetadata {
            file: None,
            source_app_icon: Some(icon),
        };
        assert!(icon_only.to_json("text").is_some());
        let constructed = PayloadMetadata::new(None, icon_only.source_app_icon.clone()).unwrap();
        let constructed_json = constructed.to_json("text").unwrap();
        assert_eq!(
            PayloadMetadata::from_json(&constructed_json, "text"),
            Some(constructed)
        );
        assert_eq!(
            PayloadMetadata::from_json(
                r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#,
                copypaste_ipc::content_type::FILE,
            )
            .and_then(|metadata| metadata.file),
            Some(file)
        );
        assert!(PayloadMetadata::from_json(
            r#"{"source_app_icon":null,"unexpected":true}"#,
            "text",
        )
        .is_none());
    }

    #[test]
    fn native_icon_dimensions_from_windows_and_android_are_preserved() {
        for edge in [16, 32, 48, 64, 96, 128] {
            let icon = SourceAppIconMetadata::new(&png(edge, edge), edge, edge)
                .expect("bounded native icons need no artificial upscaling");
            assert_eq!((icon.width, icon.height), (edge, edge));
        }
    }

    #[test]
    fn source_icon_uses_decoder_dimensions_and_bounded_base64() {
        let actual_oversize = png(129, 64);
        assert!(SourceAppIconMetadata::new(&actual_oversize, 64, 64).is_none());
        assert!(SourceAppIconMetadata::new(&png(128, 128), 128, 128).is_some());

        let oversized = format!(
            r#"{{"source_app_icon":{{"png_base64":"{}","width":64,"height":64}}}}"#,
            "A".repeat(MAX_SOURCE_APP_ICON_BASE64_BYTES + 1),
        );
        assert!(PayloadMetadata::from_json(&oversized, "text").is_none());
        assert!(PayloadMetadata::from_json(
            r#"{"source_app_icon":{"png_base64":"AAAA","width":64,"height":64}}"#,
            copypaste_ipc::content_type::FILE,
        )
        .is_none());
    }

    #[test]
    fn an_absurd_untrusted_byte_length_fails_without_allocating() {
        let key = Keyring::from_secret(&[8; 32]).item_key();
        let mut envelope = Vec::from(MAGIC.as_slice());
        envelope.push(VERSION);
        envelope.extend_from_slice(&u64::MAX.to_be_bytes());
        envelope.extend_from_slice(&[0; 32]);
        envelope.extend_from_slice(&[0; STREAM_NONCE_LEN]);

        assert!(matches!(
            open(&envelope, &key, "binary-id"),
            Err(CryptoError::AuthFailed)
        ));
    }

    #[test]
    fn obsolete_manual_format_version_is_rejected() {
        let key = Keyring::from_secret(&[8; 32]).item_key();
        let mut envelope = seal(b"old framing is not supported", &key, "version").unwrap();
        envelope[4] = 2;

        assert!(matches!(
            open(&envelope, &key, "version"),
            Err(CryptoError::AuthFailed)
        ));
    }
}
