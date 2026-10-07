//! Authenticated encryption of the complete paired-device state.

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroizing;

use super::PeerStoreError;

pub(super) const MAGIC: &[u8; 4] = b"CPP1";
const NONCE_LEN: usize = 24;
const HEADER_LEN: usize = MAGIC.len() + NONCE_LEN;
const TAG_LEN: usize = 16;
const AAD: &[u8] = b"copypaste/v2/peer-store|1";

pub(super) fn seal(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, PeerStoreError> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| PeerStoreError::Encrypt)?;
    let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    envelope.extend_from_slice(MAGIC);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

pub(super) fn open(envelope: &[u8], key: &[u8; 32]) -> Result<Zeroizing<Vec<u8>>, PeerStoreError> {
    if envelope.len() < HEADER_LEN + TAG_LEN || !envelope.starts_with(MAGIC) {
        return Err(PeerStoreError::Corrupt);
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            XNonce::from_slice(&envelope[MAGIC.len()..HEADER_LEN]),
            Payload {
                msg: &envelope[HEADER_LEN..],
                aad: AAD,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| PeerStoreError::Corrupt)
}
