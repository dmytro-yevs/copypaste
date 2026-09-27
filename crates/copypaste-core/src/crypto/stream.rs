//! STREAM constructors that keep item-key bytes inside the crypto module.

use chacha20poly1305::{
    aead::stream::{DecryptorBE32, EncryptorBE32, Nonce as StreamNonce, StreamBE32},
    Key, XChaCha20Poly1305,
};
use zeroize::Zeroizing;

use super::ItemKey;

pub(crate) const STREAM_NONCE_LEN: usize = 19;

type Nonce = StreamNonce<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>;

fn stream_key(key: &ItemKey) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(*key.0)
}

pub(crate) fn decryptor(
    key: &ItemKey,
    nonce: &[u8; STREAM_NONCE_LEN],
) -> DecryptorBE32<XChaCha20Poly1305> {
    let key_bytes = stream_key(key);
    DecryptorBE32::new(
        Key::from_slice(key_bytes.as_ref()),
        Nonce::from_slice(nonce),
    )
}

pub(crate) fn encryptor(
    key: &ItemKey,
    nonce: &[u8; STREAM_NONCE_LEN],
) -> EncryptorBE32<XChaCha20Poly1305> {
    let key_bytes = stream_key(key);
    EncryptorBE32::new(
        Key::from_slice(key_bytes.as_ref()),
        Nonce::from_slice(nonce),
    )
}
