//! STREAM constructors that keep item-key bytes inside the crypto module.

use chacha20poly1305::{
    Key, XChaCha20Poly1305,
    aead::stream::{DecryptorBE32, EncryptorBE32, Nonce as StreamNonce, StreamBE32},
};

use super::ItemKey;

pub(crate) const STREAM_NONCE_LEN: usize = 19;

type Nonce = StreamNonce<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>;

pub(crate) fn decryptor(
    key: &ItemKey,
    nonce: &[u8; STREAM_NONCE_LEN],
) -> DecryptorBE32<XChaCha20Poly1305> {
    DecryptorBE32::new(Key::from_slice(key.0.as_ref()), Nonce::from_slice(nonce))
}

pub(crate) fn encryptor(
    key: &ItemKey,
    nonce: &[u8; STREAM_NONCE_LEN],
) -> EncryptorBE32<XChaCha20Poly1305> {
    EncryptorBE32::new(Key::from_slice(key.0.as_ref()), Nonce::from_slice(nonce))
}
