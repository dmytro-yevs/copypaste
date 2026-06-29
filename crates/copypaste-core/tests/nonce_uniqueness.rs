//! Regression test for CopyPaste-49t2.
//!
//! Assert nonce uniqueness over a large number of encryption calls.
//!
//! XChaCha20-Poly1305 uses 192-bit (24-byte) random nonces. With OsRng the
//! collision probability over N encryptions is approximately N²/(2·2¹⁹²),
//! which for N = 100_000 is astronomically small (~10⁻⁴⁸). This test is a
//! practical sanity check that `encrypt_item_with_aad` calls `OsRng` correctly
//! (no zero-nonce, no repeated nonce from a PRNG fork, no deterministic
//! stub). It would catch a regression where someone swaps `OsRng` for a
//! constant or a seeded PRNG with a short cycle.

use copypaste_core::{build_item_aad, encrypt_item_with_aad, ItemId, AAD_SCHEMA_VERSION};
use std::collections::HashSet;

/// Generate N nonces via `encrypt_item_with_aad` and assert all are distinct.
#[test]
fn nonces_are_unique_over_100k_encryptions() {
    const N: usize = 100_000;

    let key = [0x42u8; 32];
    let aad = build_item_aad(&ItemId::from("test-item-id"), AAD_SCHEMA_VERSION);
    let plaintext = b"x";

    let mut seen: HashSet<[u8; 24]> = HashSet::with_capacity(N);

    for _ in 0..N {
        let (nonce, _ciphertext) =
            encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt must succeed");
        let is_new = seen.insert(nonce);
        // Fail immediately on first collision rather than collecting all N.
        assert!(
            is_new,
            "nonce collision detected after {} encryptions (CopyPaste-49t2: nonce uniqueness)",
            seen.len()
        );
    }

    // Sanity: all N nonces were inserted.
    assert_eq!(seen.len(), N);
}

/// Additionally assert that no nonce is the all-zeros sentinel, which would
/// indicate OsRng was replaced by a zero-initialised buffer.
#[test]
fn no_zero_nonces_in_1000_encryptions() {
    let key = [0xBEu8; 32];
    let aad = build_item_aad(&ItemId::from("no-zero-nonce"), AAD_SCHEMA_VERSION);
    let plaintext = b"hello";
    let zero_nonce = [0u8; 24];

    for i in 0..1_000 {
        let (nonce, _) =
            encrypt_item_with_aad(plaintext, &key, &aad).expect("encrypt must succeed");
        assert_ne!(
            nonce, zero_nonce,
            "all-zeros nonce at iteration {i} — OsRng not being used"
        );
    }
}
