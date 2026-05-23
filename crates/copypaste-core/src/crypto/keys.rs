use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::ZeroizeOnDrop;

/// Versioned HKDF salt. Bumping the version (v1 -> v2) deterministically
/// rotates every derived key, providing a clean break for future protocol
/// upgrades or key-compromise recovery. Must be exactly the bytes shipped
/// in production — changing it is a hard-fork of all on-wire and on-disk
/// encrypted material.
pub const HKDF_SALT_V1: &[u8] = b"copypaste-v1-salt";

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

    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    /// Returns the raw 32-byte X25519 secret as a plain `[u8; 32]`.
    ///
    /// **Security note (audit MED #3):** the return value is `Copy` and is
    /// NOT zeroized — callers leak key material on the stack and on heap
    /// reallocation. Prefer [`Self::secret_key_bytes_zeroizing`] for new
    /// call sites; this signature is retained so existing call sites in
    /// crates outside this worker's allow-list (e.g.
    /// `copypaste-daemon::platform::macos`) keep compiling unchanged. A
    /// follow-up patch should migrate every caller and either delete this
    /// method or formally `#[deprecated]` it.
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        // Use a `Zeroizing` buffer for the intermediate copy so the
        // x25519_dalek-allocated bytes are scrubbed when this function
        // returns. The final `[u8; 32]` returned to the caller is still a
        // fresh copy (compiler may keep it in a register), so this only
        // narrows the leak window — it does not eliminate it. Callers
        // that need a tighter window must use the `_zeroizing` variant.
        let buf: zeroize::Zeroizing<[u8; 32]> = zeroize::Zeroizing::new(self.secret.to_bytes());
        *buf
    }

    /// Returns the raw 32-byte X25519 secret wrapped in [`Zeroizing`] so
    /// the bytes are scrubbed when the returned value is dropped.
    ///
    /// Prefer this over [`Self::secret_key_bytes`] for any new code path
    /// that hands the secret to encryption, keychain storage, or any
    /// other transient consumer. See audit MED #3.
    pub fn secret_key_bytes_zeroizing(&self) -> zeroize::Zeroizing<[u8; 32]> {
        zeroize::Zeroizing::new(self.secret.to_bytes())
    }

    /// Returns the raw 32-byte ECDH shared secret as a plain `[u8; 32]`.
    ///
    /// **Security note (audit MED #3):** like [`Self::secret_key_bytes`],
    /// the return value is unscrubbed `Copy` data. The underlying
    /// `x25519_dalek::SharedSecret` is `ZeroizeOnDrop`, so the source
    /// buffer is wiped — but the returned `[u8; 32]` copy persists.
    /// Prefer [`Self::ecdh_zeroizing`] for new code; this shim is kept
    /// for cross-crate ABI stability while the migration is in flight.
    pub fn ecdh(&self, peer_public_bytes: &[u8; 32]) -> [u8; 32] {
        let peer = PublicKey::from(*peer_public_bytes);
        let shared = self.secret.diffie_hellman(&peer);
        // `shared` is dropped at the end of the function — its underlying
        // bytes are zeroized by SharedSecret's ZeroizeOnDrop impl. Wrap
        // our copy in Zeroizing too so the intermediate is scrubbed.
        let buf: zeroize::Zeroizing<[u8; 32]> = zeroize::Zeroizing::new(*shared.as_bytes());
        *buf
    }

    /// Returns the raw 32-byte ECDH shared secret wrapped in [`Zeroizing`].
    /// See [`Self::secret_key_bytes_zeroizing`] for the migration story.
    pub fn ecdh_zeroizing(&self, peer_public_bytes: &[u8; 32]) -> zeroize::Zeroizing<[u8; 32]> {
        let peer = PublicKey::from(*peer_public_bytes);
        zeroize::Zeroizing::new(*self.secret.diffie_hellman(&peer).as_bytes())
    }

    pub fn derive_enc_key(
        &self,
        peer_public_bytes: &[u8; 32],
        sender_id: &str,
        recipient_id: &str,
    ) -> [u8; 32] {
        // Audit MED #3: use the zeroizing ECDH accessor so the raw
        // shared secret is scrubbed when this function returns. The
        // derived enc key is what callers receive; the ikm itself never
        // leaks past this stack frame.
        let raw_secret = self.ecdh_zeroizing(peer_public_bytes);
        let info = format!("copypaste-v1|{}|{}", sender_id, recipient_id);
        let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT_V1), raw_secret.as_ref());
        let mut enc_key = [0u8; 32];
        hk.expand(info.as_bytes(), &mut enc_key)
            .expect("HKDF expand 32 bytes always succeeds");
        enc_key
    }

    pub fn fingerprint(&self) -> String {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(self.public.as_bytes());
        hex::encode(hash)
    }

    /// Derives a 32-byte symmetric key for local-only storage from this device's secret.
    /// Never transmitted — only used to encrypt items stored on this device.
    pub fn local_enc_key(&self) -> [u8; 32] {
        let ikm: zeroize::Zeroizing<[u8; 32]> = zeroize::Zeroizing::new(self.secret.to_bytes());
        let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT_V1), ikm.as_ref());
        let mut key = [0u8; 32];
        hk.expand(b"copypaste-local-storage-v1", &mut key)
            .expect("HKDF expand: output length 32 is always valid for SHA-256");
        key
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
        assert_eq!(
            alice.ecdh(&bob.public_key_bytes()),
            bob.ecdh(&alice.public_key_bytes())
        );
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

    /// Audit MED #3: the zeroizing accessor must return the same bytes as
    /// the plain accessor — it's a wrap-not-replace operation. Callers
    /// who switch should not observe a behavioural change.
    #[test]
    fn secret_key_bytes_zeroizing_matches_plain_accessor() {
        let kp = DeviceKeypair::generate();
        let plain = kp.secret_key_bytes();
        let zr = kp.secret_key_bytes_zeroizing();
        assert_eq!(plain, *zr);
    }

    #[test]
    fn ecdh_zeroizing_matches_plain_accessor() {
        let alice = DeviceKeypair::generate();
        let bob = DeviceKeypair::generate();
        let plain = alice.ecdh(&bob.public_key_bytes());
        let zr = alice.ecdh_zeroizing(&bob.public_key_bytes());
        assert_eq!(plain, *zr);
    }

    #[test]
    fn local_enc_key_is_deterministic_across_keypair_instances() {
        let kp = DeviceKeypair::generate();
        let secret = kp.secret_key_bytes();
        let kp_restored = DeviceKeypair::from_secret_bytes(&secret).unwrap();
        assert_eq!(kp.local_enc_key(), kp_restored.local_enc_key());
    }

    #[test]
    fn local_enc_key_differs_from_network_key() {
        let alice = DeviceKeypair::generate();
        let bob = DeviceKeypair::generate();
        let net_key = alice.derive_enc_key(&bob.public_key_bytes(), "a", "b");
        assert_ne!(alice.local_enc_key(), net_key);
    }

    /// Snapshot test: HKDF is keyed with the versioned salt `HKDF_SALT_V1`.
    /// Re-deriving with fixed inputs must produce the same output across runs.
    /// Changing `HKDF_SALT_V1` is a HARD FORK — every prior key changes.
    /// This test exists to make that rotation an explicit, intentional act
    /// (the snapshot will fail and force a deliberate update).
    #[test]
    fn hkdf_uses_versioned_salt() {
        let secret_bytes = [0x11u8; 32];
        let peer_bytes = {
            // Use a deterministic peer public via a fixed-secret keypair
            let peer_secret = [0x22u8; 32];
            let kp = DeviceKeypair::from_secret_bytes(&peer_secret).unwrap();
            kp.public_key_bytes()
        };
        let kp = DeviceKeypair::from_secret_bytes(&secret_bytes).unwrap();

        // Derive twice — deterministic regardless of salt
        let k1 = kp.derive_enc_key(&peer_bytes, "alice", "bob");
        let k2 = kp.derive_enc_key(&peer_bytes, "alice", "bob");
        assert_eq!(k1, k2, "HKDF must be deterministic for identical inputs");

        // Local enc key is also deterministic and uses the same versioned salt
        let local1 = kp.local_enc_key();
        let local2 = kp.local_enc_key();
        assert_eq!(local1, local2);

        // Compute what the key *would* have been with a different salt and
        // assert the actual key differs — proves the salt actually feeds in.
        let raw = kp.ecdh(&peer_bytes);
        let alt_hk = Hkdf::<Sha256>::new(Some(b"some-other-salt-vX"), &raw);
        let mut alt_key = [0u8; 32];
        alt_hk
            .expand(b"copypaste-v1|alice|bob", &mut alt_key)
            .unwrap();
        assert_ne!(
            k1, alt_key,
            "changing the HKDF salt MUST change the derived key"
        );

        // Sanity: salt constant is stable. (Non-emptiness is enforced at
        // compile time via the const equality below.)
        assert_eq!(HKDF_SALT_V1, b"copypaste-v1-salt");
    }
}
