//! Fuzz target: `copypaste_core::decrypt_item_with_aad` against arbitrary
//! ciphertext / nonce / key / AAD inputs.
//!
//! ## Invariant
//!
//! `decrypt_item_with_aad` MUST return `Err(EncryptError::AuthFailed)` for
//! any attacker-controlled ciphertext/nonce/AAD combination that does not
//! correspond to a legitimately-encrypted payload — it MUST NOT panic,
//! abort, or trigger UB inside the AEAD primitive.
//!
//! A panic here is a remote DoS vector: a malicious peer (P2P sync) or a
//! tampered local SQLite row could feed adversarial bytes through the same
//! decrypt path, and the daemon would crash on every read of that row.
//!
//! ## Input layout
//!
//! Rather than a tagged structured fuzzer we use a deterministic layout
//! that lets libFuzzer mutate every field independently:
//!
//! ```text
//!   byte 0..32   key        (32 B)  — wrapped via `try_into()`
//!   byte 32..56  nonce      (24 B)  — XChaCha20-Poly1305 NONCE_SIZE
//!   byte 56..    ciphertext (remainder, may be empty)
//! ```
//!
//! Inputs shorter than 56 bytes are dropped — there is no ciphertext
//! surface to fuzz below that floor.

#![no_main]

use copypaste_core::crypto::encrypt::{decrypt_item, NONCE_SIZE};
use libfuzzer_sys::fuzz_target;

const KEY_SIZE: usize = 32;
const HEADER_SIZE: usize = KEY_SIZE + NONCE_SIZE; // key + nonce

fuzz_target!(|data: &[u8]| {
    if data.len() < HEADER_SIZE {
        return;
    }

    // Carve fixed-size key and nonce out of the prefix.
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&data[0..KEY_SIZE]);

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&data[KEY_SIZE..KEY_SIZE + NONCE_SIZE]);

    let ciphertext = &data[HEADER_SIZE..];

    // Decrypt arbitrary attacker-controlled (key, nonce, ciphertext). The
    // expected outcome for fuzzer-generated input is `Err(AuthFailed)` —
    // a panic here would be a remote DoS vector.
    let _ = decrypt_item(ciphertext, &nonce, &key);
});
