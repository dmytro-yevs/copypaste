use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;

pub const NONCE_SIZE: usize = 24;
pub const TAG_SIZE: usize = 16;

#[derive(Debug, Error)]
pub enum EncryptError {
    #[error("Decryption failed: authentication tag mismatch")]
    AuthFailed,
}

/// Encrypt with XChaCha20-Poly1305. Returns (random_nonce[24], ciphertext_with_tag).
pub fn encrypt_item(plaintext: &[u8], key: &[u8; 32]) -> ([u8; NONCE_SIZE], Vec<u8>) {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .expect("XChaCha20-Poly1305 encryption cannot fail for valid inputs");
    (nonce_bytes, ciphertext)
}

/// Decrypt. Returns plaintext or AuthFailed.
pub fn decrypt_item(
    ciphertext: &[u8],
    nonce: &[u8; NONCE_SIZE],
    key: &[u8; 32],
) -> Result<Vec<u8>, EncryptError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from(*nonce);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| EncryptError::AuthFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"Hello, clipboard!";
        let (nonce, ciphertext) = encrypt_item(plaintext, &key);
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_plaintexts_produce_different_nonces() {
        let key = test_key();
        let (n1, _) = encrypt_item(b"aaa", &key);
        let (n2, _) = encrypt_item(b"aaa", &key);
        assert_ne!(n1, n2);
    }

    #[test]
    fn tampered_ciphertext_fails_decryption() {
        let key = test_key();
        let (nonce, mut ciphertext) = encrypt_item(b"secret", &key);
        ciphertext[0] ^= 0xFF;
        assert!(decrypt_item(&ciphertext, &nonce, &key).is_err());
    }

    #[test]
    fn empty_plaintext_encrypts_and_decrypts() {
        let key = test_key();
        let (nonce, ciphertext) = encrypt_item(b"", &key);
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, b"");
    }

    #[test]
    fn large_plaintext_1mb_roundtrip() {
        let key = test_key();
        let plaintext = vec![0xABu8; 1_000_000];
        let (nonce, ciphertext) = encrypt_item(&plaintext, &key);
        let decrypted = decrypt_item(&ciphertext, &nonce, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
