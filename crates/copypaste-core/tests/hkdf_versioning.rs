//! HKDF salt versioning regression tests.
//!
//! Background: `crypto::keys` derives all symmetric keys via HKDF-SHA256
//! using a versioned salt constant (`HKDF_SALT_V1 = b"copypaste-v1-salt"`).
//! Bumping that salt to V2 is intended as a HARD FORK — every prior key
//! must change deterministically. These tests pin the contract:
//!
//!   1. Same inputs + same salt version → identical key bytes (determinism).
//!   2. Same inputs + DIFFERENT salt version → different key bytes (rotation).
//!   3. Derived key length matches XChaCha20-Poly1305 requirement (32 bytes).
//!   4. (Future) An on-disk ciphertext produced with V1 keys must not decrypt
//!      with V2 keys — migration helper is required and currently absent.
//!
//! Note: `HKDF_SALT_V1` is `pub` inside `crypto::keys` but the `keys` module
//! is private (`mod keys;`), so integration tests cannot import it directly.
//! We re-derive the HKDF independently using the documented salt bytes to
//! verify the rotation property end-to-end via the public API.

use copypaste_core::{decrypt_item, encrypt_item, DeviceKeypair, NONCE_SIZE};
use hkdf::Hkdf;
use sha2::Sha256;

/// Documented production salt constant. Mirrors `crypto::keys::HKDF_SALT_V1`.
/// If this string ever drifts from the source, the determinism tests below
/// will fail loudly — that is the intended early-warning signal.
const HKDF_SALT_V1_BYTES: &[u8] = b"copypaste-v1-salt";

/// Hypothetical V2 salt. Used purely to prove the salt actually feeds into
/// the KDF — swapping it MUST change every derived byte.
const HKDF_SALT_V2_CANDIDATE: &[u8] = b"copypaste-v2-salt";

/// Network-key info string format used by `DeviceKeypair::derive_enc_key`.
fn network_info(sender_id: &str, recipient_id: &str) -> String {
    format!("copypaste-v1|{}|{}", sender_id, recipient_id)
}

/// Independently re-derive what `derive_enc_key` would produce under an
/// arbitrary salt. Mirrors the in-crate HKDF call so we can swap V1 ↔ V2.
fn derive_with_salt(
    me: &DeviceKeypair,
    peer_public: &[u8; 32],
    sender_id: &str,
    recipient_id: &str,
    salt: &[u8],
) -> [u8; 32] {
    let raw_secret = me.ecdh(peer_public);
    let info = network_info(sender_id, recipient_id);
    let hk = Hkdf::<Sha256>::new(Some(salt), &raw_secret);
    let mut out = [0u8; 32];
    hk.expand(info.as_bytes(), &mut out)
        .expect("HKDF expand 32 bytes always succeeds");
    out
}

/// Fixed-secret keypair factory — deterministic across test runs.
fn fixed_keypair(seed: u8) -> DeviceKeypair {
    let secret = [seed; 32];
    DeviceKeypair::from_secret_bytes(&secret).unwrap()
}

// ---------------------------------------------------------------------------
// 1. Salt version regression: V1 ≠ V2 for identical inputs
// ---------------------------------------------------------------------------

#[test]
fn salt_v1_and_v2_derive_different_keys_from_same_input() {
    let alice = fixed_keypair(0x11);
    let bob = fixed_keypair(0x22);
    let bob_pub = bob.public_key_bytes();

    let key_v1 = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V1_BYTES);
    let key_v2 = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V2_CANDIDATE);

    assert_ne!(
        key_v1, key_v2,
        "bumping the HKDF salt (v1 → v2) MUST rotate the derived key — \
         this is the entire point of versioning the salt"
    );

    // The public API call must match the V1 path byte-for-byte (proves the
    // production code actually uses HKDF_SALT_V1_BYTES, not some other salt).
    let key_via_api = alice.derive_enc_key(&bob_pub, "alice", "bob");
    assert_eq!(
        key_via_api, key_v1,
        "DeviceKeypair::derive_enc_key MUST use HKDF_SALT_V1 \
         (b\"copypaste-v1-salt\") — if this fails, the production salt \
         constant has drifted from the documented value"
    );
}

// ---------------------------------------------------------------------------
// 2. Determinism: same inputs + same salt → identical bytes
// ---------------------------------------------------------------------------

#[test]
fn same_salt_version_is_deterministic() {
    let alice = fixed_keypair(0x33);
    let bob = fixed_keypair(0x44);
    let bob_pub = bob.public_key_bytes();

    // Public API path — call twice
    let api_a = alice.derive_enc_key(&bob_pub, "device-a", "device-b");
    let api_b = alice.derive_enc_key(&bob_pub, "device-a", "device-b");
    assert_eq!(
        api_a, api_b,
        "derive_enc_key must be deterministic for identical inputs"
    );

    // Local storage key path — call twice
    let local_a = alice.local_enc_key();
    let local_b = alice.local_enc_key();
    assert_eq!(
        local_a, local_b,
        "local_enc_key must be deterministic for the same DeviceKeypair"
    );

    // Cross-instance determinism: restore from bytes, re-derive, identical
    let restored = DeviceKeypair::from_secret_bytes(&[0x33u8; 32]).unwrap();
    assert_eq!(
        restored.local_enc_key(),
        local_a,
        "local_enc_key must be stable across DeviceKeypair instances \
         constructed from the same secret bytes"
    );
}

// ---------------------------------------------------------------------------
// 3. Output length contract: 32 bytes for XChaCha20-Poly1305
// ---------------------------------------------------------------------------

#[test]
fn key_length_matches_xchacha_requirement_32_bytes() {
    let alice = fixed_keypair(0x55);
    let bob = fixed_keypair(0x66);

    let net_key = alice.derive_enc_key(&bob.public_key_bytes(), "x", "y");
    assert_eq!(
        net_key.len(),
        32,
        "XChaCha20-Poly1305 requires exactly 32-byte keys"
    );

    let local_key = alice.local_enc_key();
    assert_eq!(local_key.len(), 32, "local_enc_key must be 32 bytes");

    // Round-trip sanity: the derived key actually works with the AEAD layer.
    let plaintext = b"hkdf-versioning-roundtrip";
    let (nonce, ct) = encrypt_item(plaintext, &net_key).unwrap();
    assert_eq!(nonce.len(), NONCE_SIZE);
    let pt = decrypt_item(&ct, &nonce, &net_key).unwrap();
    assert_eq!(pt, plaintext);
}

// ---------------------------------------------------------------------------
// 4. Migration helper — NOT YET IMPLEMENTED
// ---------------------------------------------------------------------------

/// API gap: no V2 salt constant, no `derive_enc_key_with_version()`, and no
/// migration helper that re-encrypts V1 ciphertext under a V2 key. When V2
/// is introduced this test must be un-`ignore`d and updated to call the real
/// migration API. Until then it documents the missing surface.
///
/// Expected future API shape (subject to design review):
///   ```ignore
///   pub const HKDF_SALT_V2: &[u8] = b"copypaste-v2-salt";
///   impl DeviceKeypair {
///       pub fn derive_enc_key_v2(&self, peer: &[u8;32], s: &str, r: &str) -> [u8;32];
///       pub fn local_enc_key_v2(&self) -> [u8;32];
///   }
///   pub fn migrate_ciphertext_v1_to_v2(
///       ct: &[u8], nonce: &[u8; NONCE_SIZE],
///       v1_key: &[u8; 32], v2_key: &[u8; 32],
///   ) -> Result<(Vec<u8>, [u8; NONCE_SIZE]), EncryptError>;
///   ```
#[test]
#[ignore = "TODO: HKDF_SALT_V2 + derive_enc_key_v2 + migrate_ciphertext_v1_to_v2 \
            not yet implemented in copypaste-core. Un-ignore when V2 lands."]
fn migration_old_v1_ciphertext_decryptable_with_v1_key_only() {
    let alice = fixed_keypair(0x77);
    let bob = fixed_keypair(0x88);
    let bob_pub = bob.public_key_bytes();

    let v1_key = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V1_BYTES);
    let v2_key = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V2_CANDIDATE);

    let plaintext = b"legacy v1 ciphertext";
    let (nonce, ct) = encrypt_item(plaintext, &v1_key).unwrap();

    // Sanity: v1 key decrypts the v1 ciphertext.
    let pt = decrypt_item(&ct, &nonce, &v1_key).unwrap();
    assert_eq!(pt, plaintext);

    // The real contract: v2 key MUST NOT decrypt v1 ciphertext (auth tag
    // mismatch). This is what forces a migration step rather than a silent
    // key swap.
    assert!(
        decrypt_item(&ct, &nonce, &v2_key).is_err(),
        "v2-derived key must NOT decrypt v1-derived ciphertext — \
         a migration step is mandatory"
    );

    // TODO: once `migrate_ciphertext_v1_to_v2` exists, assert:
    //   let (new_ct, new_nonce) =
    //       migrate_ciphertext_v1_to_v2(&ct, &nonce, &v1_key, &v2_key).unwrap();
    //   assert_eq!(decrypt_item(&new_ct, &new_nonce, &v2_key).unwrap(), plaintext);
    //   assert!(decrypt_item(&new_ct, &new_nonce, &v1_key).is_err());
}
