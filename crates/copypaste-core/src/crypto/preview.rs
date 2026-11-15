//! Authenticated cache entries use a domain-separated key and bind their source.
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, Rng};
use zeroize::Zeroizing;

use super::{CryptoError, PreviewKey, NONCE_LEN, TAG_LEN};

// Bump this domain whenever the semantic meaning of the cached preview changes.
fn aad(id: &str, fingerprint: &[u8; 32]) -> Vec<u8> {
    let mut aad = b"copypaste/v2/history-preview-v1|".to_vec();
    aad.extend_from_slice(&(id.len() as u64).to_be_bytes());
    aad.extend_from_slice(id.as_bytes());
    aad.extend_from_slice(fingerprint);
    aad
}

pub(crate) fn seal_preview(
    plaintext: &[u8],
    key: &PreviewKey,
    id: &str,
    fingerprint: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));
    let nonce: [u8; NONCE_LEN] = OsRng.gen();
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad(id, fingerprint),
            },
        )
        .map_err(|_| CryptoError::Internal("AEAD rejected the preview"))?;
    Ok((nonce.to_vec(), ciphertext))
}

pub(crate) fn open_preview(
    ciphertext: &[u8],
    nonce: &[u8],
    key: &PreviewKey,
    id: &str,
    fingerprint: &[u8; 32],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if nonce.len() != NONCE_LEN || ciphertext.len() < TAG_LEN {
        return Err(CryptoError::AuthFailed);
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.0.as_ref()));
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &aad(id, fingerprint),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::AuthFailed)
}
