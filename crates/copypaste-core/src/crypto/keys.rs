use hkdf::Hkdf;
use sha2::{Sha256, Sha512};
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::ZeroizeOnDrop;

/// Versioned HKDF salt. Bumping the version (v1 -> v2) deterministically
/// rotates every derived key, providing a clean break for future protocol
/// upgrades or key-compromise recovery. Must be exactly the bytes shipped
/// in production — changing it is a hard-fork of all on-wire and on-disk
/// encrypted material.
pub const HKDF_SALT_V1: &[u8] = b"copypaste-v1-salt";

/// Current HKDF derivation version. Bumped from 1 → 2 in v0.3 (T5):
///
/// **v1** keyed HKDF-SHA256 with a single static salt (`HKDF_SALT_V1`) and an
/// `info` of `copypaste-v1|{sender}|{recipient}` (network) or
/// `copypaste-local-storage-v1` (local). Every device used the same salt; key
/// rotation required a full hard-fork bump of `HKDF_SALT_V1`.
///
/// **v2** keys HKDF-SHA512 with a *per-device-pair* salt derived from the pair
/// fingerprint, and an `info` string of
/// `copypaste-hkdf-v2|{pair_id}|{key_purpose}` where `key_purpose ∈
/// {"storage", "sync", "telemetry"}`. This means each device-pair can rotate
/// its own keys independently (re-issue `pair_id`) without affecting any other
/// pair, and the three logical key purposes are domain-separated by construction.
///
/// v1 derivation is kept (`derive_enc_key` / `local_enc_key`) ONLY for the
/// ciphertext-migration sweep (v3 → v4 schema). All NEW encryption MUST use
/// the v2 family.
pub const HKDF_VERSION: u32 = 2;

/// Base HKDF salt prefix for v2 derivations. The actual salt fed to HKDF is
/// `SHA-256(HKDF_SALT_V2_BASE || pair_id_bytes)` — using SHA-256 of the
/// concatenation gives us a fixed-length 32-byte salt regardless of the
/// `pair_id` shape (UUID string, fingerprint hex, etc.).
pub const HKDF_SALT_V2_BASE: &[u8] = b"copypaste-v2-salt";

/// Compute the v2 per-pair salt: `SHA-256(HKDF_SALT_V2_BASE || pair_id)`.
/// Exposed for tests and the migration helper that needs to verify two
/// pair-ids produce different salts.
pub fn hkdf_v2_pair_salt(pair_id: &str) -> [u8; 32] {
    use sha2::Digest;
    let mut h = Sha256::new();
    h.update(HKDF_SALT_V2_BASE);
    h.update(pair_id.as_bytes());
    let out = h.finalize();
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&out);
    salt
}

/// Derive a 32-byte v2 key from `ikm` (input keying material — typically the
/// raw ECDH output or the device's secret bytes), bound to `pair_id` (per-pair
/// salt) and `purpose` ∈ {"storage", "sync", "telemetry"}.
///
/// **Deterministic** for identical `(ikm, pair_id, purpose)`. Domain-separated
/// from v1 by both algorithm (SHA-512 vs SHA-256) and `info` prefix
/// (`copypaste-hkdf-v2|...`).
fn derive_key_v2(ikm: &[u8], pair_id: &str, purpose: &str) -> [u8; 32] {
    let salt = hkdf_v2_pair_salt(pair_id);
    let info = format!("copypaste-hkdf-v2|{}|{}", pair_id, purpose);
    let hk = Hkdf::<Sha512>::new(Some(&salt), ikm);
    let mut key = [0u8; 32];
    hk.expand(info.as_bytes(), &mut key)
        .expect("HKDF-SHA512 expand 32 bytes always succeeds");
    key
}

/// v2 storage-key derivation. Used for local at-rest item encryption.
/// Replaces `DeviceKeypair::local_enc_key()` for new ciphertexts.
pub fn derive_storage_key_v2(ikm: &[u8], pair_id: &str) -> [u8; 32] {
    derive_key_v2(ikm, pair_id, "storage")
}

/// v2 sync-key derivation. Used for over-the-wire item payload encryption
/// between paired devices. Replaces `DeviceKeypair::derive_enc_key()` for
/// new ciphertexts.
pub fn derive_sync_key_v2(ikm: &[u8], pair_id: &str) -> [u8; 32] {
    derive_key_v2(ikm, pair_id, "sync")
}

/// v2 telemetry-key derivation. Reserved for future use (e.g. authenticated
/// metric submission to the relay). Domain-separated from storage/sync so a
/// telemetry key leak cannot decrypt clipboard data.
pub fn derive_telemetry_key_v2(ikm: &[u8], pair_id: &str) -> [u8; 32] {
    derive_key_v2(ikm, pair_id, "telemetry")
}

/// Fixed 32-byte HKDF salt for the local-storage key-version-2 derivation path.
///
/// Computed as `SHA-256(b"copypaste/storage-key/v2/hkdf-salt")` and pinned as
/// a literal so it can never drift from what is written on disk. Any change
/// here is a hard-fork of all `key_version = 2` local-storage ciphertexts.
///
/// This constant is distinct from [`HKDF_SALT_V2_BASE`] (which is a *prefix*
/// used to derive per-pair salts for sync/telemetry keys). This one is used
/// exclusively by [`derive_v2`] for the single-device local-storage path where
/// there is no `pair_id`.
pub(crate) const HKDF_SALT_V2: &[u8; 32] = &[
    0xdd, 0x4e, 0xb4, 0x9c, 0x1e, 0x2e, 0x3c, 0x66,
    0x11, 0xa4, 0x1b, 0x03, 0x3c, 0xea, 0x9a, 0x50,
    0x5c, 0x91, 0xa3, 0x09, 0x09, 0xa6, 0x67, 0xbb,
    0x3f, 0x42, 0xb3, 0xd7, 0xf3, 0x33, 0x02, 0x8e,
];

/// Derive a 32-byte local-storage key from a raw 32-byte seed using the v2
/// HKDF family.
///
/// Uses HKDF-SHA512 with the frozen [`HKDF_SALT_V2`] salt and the info string
/// `"copypaste-local-storage-v2"`. Domain-separated from the network sync/
/// telemetry keys by the `info` string and from v1 by both the algorithm
/// (SHA-512 vs SHA-256) and the salt bytes.
///
/// This is the single-device equivalent of [`derive_storage_key_v2`]: it does
/// NOT take a `pair_id` because local-storage encryption has no peer concept.
pub fn derive_v2(seed: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha512>::new(Some(HKDF_SALT_V2), seed);
    let mut key = [0u8; 32];
    hk.expand(b"copypaste-local-storage-v2", &mut key)
        .expect("HKDF-SHA512 expand 32 bytes always succeeds");
    key
}

/// v1 local-storage-key derivation, exposed as a free function so the
/// migration sweep can derive the legacy key without going through the
/// `DeviceKeypair` instance API. Identical output to
/// `DeviceKeypair::local_enc_key()`. **Migration-only** — do NOT use for
/// new ciphertexts.
pub fn derive_storage_key_v1(ikm: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT_V1), ikm);
    let mut key = [0u8; 32];
    hk.expand(b"copypaste-local-storage-v1", &mut key)
        .expect("HKDF expand: output length 32 is always valid for SHA-256");
    key
}

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

    /// Returns the raw 32-byte X25519 secret wrapped in [`zeroize::Zeroizing`] so
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

    /// Returns the raw 32-byte ECDH shared secret wrapped in [`zeroize::Zeroizing`].
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
    pub fn local_enc_key(&self) -> zeroize::Zeroizing<[u8; 32]> {
        let ikm: zeroize::Zeroizing<[u8; 32]> = zeroize::Zeroizing::new(self.secret.to_bytes());
        let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT_V1), ikm.as_ref());
        let mut key = [0u8; 32];
        hk.expand(b"copypaste-local-storage-v1", &mut key)
            .expect("HKDF expand: output length 32 is always valid for SHA-256");
        zeroize::Zeroizing::new(key)
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
        assert_ne!(*alice.local_enc_key(), net_key);
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

    // ---------------------------------------------------------------------
    // T5 (v0.3): HKDF v2 — per-pair salt, SHA-512, purpose domain separation
    // ---------------------------------------------------------------------

    /// v1 and v2 derivations must produce different keys even with identical
    /// IKM. Domain-separation is the whole point of bumping the HKDF version;
    /// if these collided we'd have a silent migration footgun.
    #[test]
    fn hkdf_v1_and_v2_produce_different_keys() {
        let ikm = [0x33u8; 32];
        let v1 = derive_storage_key_v1(&ikm);
        let v2 = derive_storage_key_v2(&ikm, "pair-abc");
        assert_ne!(v1, v2, "v1 and v2 HKDF derivations must NOT collide");
    }

    /// Two different `pair_id`s must produce two different v2 keys for the
    /// same IKM and purpose. This is the property that lets each device-pair
    /// rotate independently.
    #[test]
    fn hkdf_v2_different_pair_ids_produce_different_keys() {
        let ikm = [0x44u8; 32];
        let ka = derive_storage_key_v2(&ikm, "pair-aaa");
        let kb = derive_storage_key_v2(&ikm, "pair-bbb");
        assert_ne!(ka, kb, "different pair_ids must derive different keys");
    }

    /// Same `(ikm, pair_id)` must give the same key — re-derivable across
    /// process restarts.
    #[test]
    fn hkdf_v2_is_deterministic() {
        let ikm = [0x55u8; 32];
        let k1 = derive_storage_key_v2(&ikm, "pair-zzz");
        let k2 = derive_storage_key_v2(&ikm, "pair-zzz");
        assert_eq!(k1, k2);
    }

    /// The three purposes (storage, sync, telemetry) must be mutually
    /// domain-separated. A storage-key leak must NOT enable decryption of
    /// sync traffic, and vice-versa.
    #[test]
    fn hkdf_v2_purposes_are_domain_separated() {
        let ikm = [0x66u8; 32];
        let pair_id = "pair-domain-test";
        let s = derive_storage_key_v2(&ikm, pair_id);
        let n = derive_sync_key_v2(&ikm, pair_id);
        let t = derive_telemetry_key_v2(&ikm, pair_id);
        assert_ne!(s, n);
        assert_ne!(s, t);
        assert_ne!(n, t);
    }

    /// The per-pair salt itself must differ between pair_ids — guards against
    /// a refactor that accidentally drops `pair_id` from the salt input.
    #[test]
    fn hkdf_v2_pair_salt_varies_by_pair_id() {
        let a = hkdf_v2_pair_salt("pair-a");
        let b = hkdf_v2_pair_salt("pair-b");
        assert_ne!(a, b);
        // Determinism check: same pair_id → same salt
        assert_eq!(hkdf_v2_pair_salt("pair-a"), a);
    }

    /// HKDF version constant is locked at 2 for the v0.3 release. A bump to 3
    /// would be a hard fork and should require an explicit, deliberate change
    /// to this snapshot test.
    #[test]
    fn hkdf_version_is_2() {
        assert_eq!(HKDF_VERSION, 2);
    }

    // -------------------------------------------------------------------------
    // wave1a-atomic: HKDF_SALT_V2 golden-byte test + derive_v2 tests
    // -------------------------------------------------------------------------

    /// T4 (golden-file): `HKDF_SALT_V2` must be exactly the SHA-256 of the
    /// canonical input string `"copypaste/storage-key/v2/hkdf-salt"`. Changing
    /// these bytes is a hard-fork of every `key_version = 2` local-storage
    /// ciphertext — this test makes that a deliberate, visible act.
    #[test]
    fn hkdf_salt_v2_is_sha256_of_canonical_input() {
        use sha2::Digest;
        let expected = Sha256::digest(b"copypaste/storage-key/v2/hkdf-salt");
        assert_eq!(
            HKDF_SALT_V2.as_ref(),
            expected.as_slice(),
            "HKDF_SALT_V2 bytes must equal SHA-256(b\"copypaste/storage-key/v2/hkdf-salt\")"
        );
    }

    /// `derive_v2` must be deterministic: same seed → same key.
    #[test]
    fn derive_v2_is_deterministic() {
        let seed = [0xA1u8; 32];
        let k1 = derive_v2(&seed);
        let k2 = derive_v2(&seed);
        assert_eq!(k1, k2);
    }

    /// `derive_v2` must produce a different key than `derive_storage_key_v1`
    /// for the same IKM — domain separation between v1 and v2 is critical.
    #[test]
    fn derive_v2_differs_from_v1() {
        let seed = [0xB2u8; 32];
        let v1 = derive_storage_key_v1(&seed);
        let v2 = derive_v2(&seed);
        assert_ne!(v1, v2, "derive_v2 must not collide with derive_storage_key_v1");
    }

    /// `derive_v2` must produce a different key than `derive_storage_key_v2`
    /// even for the same seed — they use different salt constructions.
    #[test]
    fn derive_v2_differs_from_derive_storage_key_v2() {
        let seed = [0xC3u8; 32];
        let local_v2 = derive_v2(&seed);
        let pair_v2 = derive_storage_key_v2(&seed, "some-pair-id");
        assert_ne!(
            local_v2, pair_v2,
            "local derive_v2 must not collide with per-pair derive_storage_key_v2"
        );
    }

    /// Different seeds must produce different keys.
    #[test]
    fn derive_v2_different_seeds_produce_different_keys() {
        let k1 = derive_v2(&[0x01u8; 32]);
        let k2 = derive_v2(&[0x02u8; 32]);
        assert_ne!(k1, k2);
    }

    /// `derive_storage_key_v1` (free function) and `DeviceKeypair::local_enc_key`
    /// (instance method) must produce the same key for the same secret bytes —
    /// the free function exists only as a re-export for the migration sweep
    /// and must NOT subtly diverge from the legacy path.
    #[test]
    fn derive_storage_key_v1_matches_local_enc_key() {
        let kp = DeviceKeypair::generate();
        let secret = kp.secret_key_bytes();
        let instance_key = kp.local_enc_key();
        let free_fn_key = derive_storage_key_v1(&secret);
        assert_eq!(*instance_key, free_fn_key);
    }
}
