use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::ZeroizeOnDrop;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("Invalid secret key bytes (expected 32)")]
    InvalidLength,
}

#[derive(ZeroizeOnDrop)]
pub struct DeviceKeypair {
    secret: StaticSecret,
    public: PublicKey,
}

impl DeviceKeypair {
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self { secret, public }
    }

    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Result<Self, KeyError> {
        let secret = StaticSecret::from(*bytes);
        let public = PublicKey::from(&secret);
        Ok(Self { secret, public })
    }

    pub fn public_key_bytes(&self) -> [u8; 32] { *self.public.as_bytes() }

    pub fn secret_key_bytes(&self) -> [u8; 32] { self.secret.to_bytes() }

    pub fn ecdh(&self, peer_public_bytes: &[u8; 32]) -> [u8; 32] {
        let peer = PublicKey::from(*peer_public_bytes);
        *self.secret.diffie_hellman(&peer).as_bytes()
    }

    pub fn derive_enc_key(&self, peer_public_bytes: &[u8; 32], sender_id: &str, recipient_id: &str) -> [u8; 32] {
        let raw_secret = self.ecdh(peer_public_bytes);
        let info = format!("copypaste-v1|{}|{}", sender_id, recipient_id);
        let hk = Hkdf::<Sha256>::new(None, &raw_secret);
        let mut enc_key = [0u8; 32];
        hk.expand(info.as_bytes(), &mut enc_key).expect("HKDF expand 32 bytes always succeeds");
        enc_key
    }

    pub fn fingerprint(&self) -> String {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(self.public.as_bytes());
        hex::encode(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_generates_unique_keys() {
        let kp1 = DeviceKeypair::generate();
        let kp2 = DeviceKeypair::generate();
        assert_ne!(kp1.public_key_bytes(), kp2.public_key_bytes());
    }

    #[test]
    fn ecdh_shared_secret_is_symmetric() {
        let alice = DeviceKeypair::generate();
        let bob = DeviceKeypair::generate();
        assert_eq!(alice.ecdh(&bob.public_key_bytes()), bob.ecdh(&alice.public_key_bytes()));
    }

    #[test]
    fn derive_enc_key_is_deterministic() {
        let alice = DeviceKeypair::generate();
        let bob = DeviceKeypair::generate();
        let key1 = alice.derive_enc_key(&bob.public_key_bytes(), "a-id", "b-id");
        let key2 = alice.derive_enc_key(&bob.public_key_bytes(), "a-id", "b-id");
        assert_eq!(key1, key2);
    }

    #[test]
    fn derive_enc_key_differs_for_different_device_ids() {
        let alice = DeviceKeypair::generate();
        let bob = DeviceKeypair::generate();
        let key1 = alice.derive_enc_key(&bob.public_key_bytes(), "a-id", "b-id");
        let key2 = alice.derive_enc_key(&bob.public_key_bytes(), "b-id", "a-id");
        assert_ne!(key1, key2);
    }

    #[test]
    fn public_key_fingerprint_is_64_chars_hex() {
        let kp = DeviceKeypair::generate();
        let fp = kp.fingerprint();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn keypair_roundtrips_through_bytes() {
        let kp = DeviceKeypair::generate();
        let secret_bytes = kp.secret_key_bytes();
        let restored = DeviceKeypair::from_secret_bytes(&secret_bytes).unwrap();
        assert_eq!(kp.public_key_bytes(), restored.public_key_bytes());
    }
}
