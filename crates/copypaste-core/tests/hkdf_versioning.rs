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

use copypaste_core::{
    build_item_aad, decrypt_item_with_aad, encrypt_item_with_aad, DeviceKeypair,
    AAD_SCHEMA_VERSION, NONCE_SIZE,
};
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
///
/// CopyPaste-lkmy: length-prefix each ID so adversarial device IDs containing
/// `|` cannot collide across the sender/recipient boundary.
fn network_info(sender_id: &str, recipient_id: &str) -> String {
    format!(
        "copypaste-v1|{}:{}|{}:{}",
        sender_id.len(),
        sender_id,
        recipient_id.len(),
        recipient_id
    )
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
    // CopyPaste-1qht: ecdh() now returns Zeroizing<[u8;32]>; use as_ref() for HKDF.
    let hk = Hkdf::<Sha256>::new(Some(salt), raw_secret.as_ref());
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
        *key_via_api, key_v1,
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
    let aad = build_item_aad("hkdf-roundtrip", AAD_SCHEMA_VERSION);
    let (nonce, ct) = encrypt_item_with_aad(plaintext, &net_key, &aad).unwrap();
    assert_eq!(nonce.len(), NONCE_SIZE);
    let pt = decrypt_item_with_aad(&ct, &nonce, &net_key, &aad).unwrap();
    assert_eq!(pt, plaintext);
}

// ---------------------------------------------------------------------------
// 4. V1 ↔ V2 key isolation — currently provable invariants
// ---------------------------------------------------------------------------

/// CopyPaste-e4a0: pin the two currently-verifiable key-rotation invariants:
///   (a) A V1-salt-derived key correctly decrypts V1 ciphertext.
///   (b) A V2-salt-derived key is rejected by V1 ciphertext (auth-tag mismatch).
///
/// These assertions do NOT require `migrate_ciphertext_v1_to_v2` (which does
/// not yet exist). They prove the AEAD layer enforces key isolation, making a
/// proper migration step mandatory when HKDF V2 eventually lands.
///
/// The future migration helper (`migrate_ciphertext_v1_to_v2`) will need its
/// own test once it is implemented. Expected API shape (tracked separately):
///   ```text
///   pub const HKDF_SALT_V2: &[u8] = b"copypaste-v2-salt";
///   pub fn migrate_ciphertext_v1_to_v2(
///       ct: &[u8], nonce: &[u8; NONCE_SIZE],
///       v1_key: &[u8; 32], v2_key: &[u8; 32],
///   ) -> Result<(Vec<u8>, [u8; NONCE_SIZE]), EncryptError>;
///   ```
#[test]
fn v1_key_isolates_from_v2_salt_derived_key() {
    let alice = fixed_keypair(0x77);
    let bob = fixed_keypair(0x88);
    let bob_pub = bob.public_key_bytes();

    let v1_key = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V1_BYTES);
    let v2_key = derive_with_salt(&alice, &bob_pub, "alice", "bob", HKDF_SALT_V2_CANDIDATE);

    let plaintext = b"legacy v1 ciphertext";
    let aad = build_item_aad("hkdf-migration", AAD_SCHEMA_VERSION);
    let (nonce, ct) = encrypt_item_with_aad(plaintext, &v1_key, &aad).unwrap();

    // (a) V1 key decrypts V1 ciphertext — round-trip regression.
    let pt = decrypt_item_with_aad(&ct, &nonce, &v1_key, &aad).unwrap();
    assert_eq!(pt, plaintext);

    // (b) V2-salt key MUST NOT decrypt V1 ciphertext (auth-tag mismatch).
    // This is the invariant that makes a migration step mandatory; a silent
    // key swap would silently corrupt all stored ciphertexts.
    assert!(
        decrypt_item_with_aad(&ct, &nonce, &v2_key, &aad).is_err(),
        "v2-derived key must NOT decrypt v1-derived ciphertext — \
         a migration step is mandatory"
    );
}
