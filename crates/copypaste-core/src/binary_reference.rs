//! Row-authenticated references to shared encrypted binary payloads.
use crate::{CryptoError, ItemKey};
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"CPR1";
const NONCE_BYTES: usize = 24;
const PREFIX_BYTES: usize = 4 + NONCE_BYTES + 4;
pub(crate) const IMPORT_ID_PREFIX: &str = "file-import-";

pub(crate) fn seal(
    bytes: &[u8],
    digest: &[u8; 32],
    key: &ItemKey,
    id: &str,
) -> Result<Vec<u8>, CryptoError> {
    let blob_id = crate::binary::content_hash(digest);
    let (nonce, reference) = crate::encrypt(blob_id.as_bytes(), key, id)?;
    let blob = crate::binary::seal_with_digest(bytes, digest, key, &blob_id)?;
    let mut envelope = Vec::with_capacity(PREFIX_BYTES + reference.len() + blob.len());
    envelope.extend_from_slice(MAGIC);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&(reference.len() as u32).to_be_bytes());
    envelope.extend_from_slice(&reference);
    envelope.extend_from_slice(&blob);
    Ok(envelope)
}

pub(crate) fn split(envelope: &[u8]) -> Option<(&[u8], &[u8])> {
    if !envelope.starts_with(MAGIC) {
        return None;
    }
    let len = u32::from_be_bytes(envelope.get(28..32)?.try_into().ok()?) as usize;
    // A SHA-256 hex reference plus the AEAD tag. Reject ambiguous frames.
    if len != 64 + crate::crypto::TAG_LEN {
        return None;
    }
    let end = PREFIX_BYTES.checked_add(len)?;
    Some((envelope.get(..end)?, envelope.get(end..)?))
}

pub(crate) fn is_reference(envelope: &[u8]) -> bool {
    envelope.starts_with(MAGIC)
}

pub(crate) fn open(
    envelope: &[u8],
    key: &ItemKey,
    id: &str,
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (reference, blob) = split(envelope).ok_or(CryptoError::AuthFailed)?;
    let blob_id = crate::decrypt(&reference[PREFIX_BYTES..], &reference[4..28], key, id)?;
    let blob_id = std::str::from_utf8(&blob_id).map_err(|_| CryptoError::AuthFailed)?;
    if blob_id.len() != 64
        || !blob_id.bytes().all(|c| c.is_ascii_hexdigit())
        || !blob.starts_with(b"CPB2")
    {
        return Err(CryptoError::AuthFailed);
    }
    let bytes = crate::binary::open(blob, key, blob_id)?;
    if crate::binary::content_hash(&crate::binary::content_digest(&bytes)) != blob_id {
        return Err(CryptoError::AuthFailed);
    }
    Ok(bytes)
}
