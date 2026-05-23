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
//!   byte 0..32                  key        (32 B)  — wrapped via `try_into()`
//!   byte 32..56                 nonce      (24 B)  — XChaCha20-Poly1305 NONCE_SIZE
//!   byte 56                     aad_len    (1 B)   — 0..=255 bytes of AAD
//!   byte 57..57+aad_len         aad        (variable, optional)
//!   byte 57+aad_len..           ciphertext (remainder, may be empty)
//! ```
//!
//! Inputs shorter than 57 bytes are dropped — there is no AAD-length byte
//! to read below that floor. If `aad_len` exceeds the bytes remaining we
//! clamp it to whatever is left and treat the rest as empty ciphertext —
//! this still exercises the AEAD primitive with a truthfully-attached AAD.

#![no_main]

use copypaste_core::crypto::encrypt::{decrypt_item, decrypt_item_with_aad, NONCE_SIZE};
use libfuzzer_sys::fuzz_target;

const KEY_SIZE: usize = 32;
/// key (32) + nonce (24) + aad_len byte (1) = 57.
const HEADER_SIZE: usize = KEY_SIZE + NONCE_SIZE + 1;

fuzz_target!(|data: &[u8]| {
    if data.len() < HEADER_SIZE {
        return;
    }

    // Carve fixed-size key and nonce out of the prefix.
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&data[0..KEY_SIZE]);

    let mut nonce = [0u8; NONCE_SIZE];
    nonce.copy_from_slice(&data[KEY_SIZE..KEY_SIZE + NONCE_SIZE]);

    // AAD length byte controls how much of the remaining suffix is treated
    // as Additional Authenticated Data. Clamping to remaining length means
    // the fuzzer can explore zero-AAD, small-AAD, and AAD-dominates-input
    // shapes from one corpus entry.
    let aad_byte = data[KEY_SIZE + NONCE_SIZE] as usize;
    let suffix = &data[HEADER_SIZE..];
    let aad_len = aad_byte.min(suffix.len());
    let (aad, ciphertext) = suffix.split_at(aad_len);

    // Two parallel surfaces — both share the same XChaCha20-Poly1305
    // primitive but reach it through different wrappers. Fuzz both so we
    // catch any wrapper-side panic (slice indexing in AAD setup, payload
    // construction, etc.) regardless of which one a future call site picks.
    let _ = decrypt_item_with_aad(ciphertext, &nonce, &key, aad);

    // The no-AAD wrapper (legacy/back-compat path) ignores `aad` and is
    // equivalent to `decrypt_item_with_aad(.., &[])`. Re-running it on the
    // same ciphertext costs almost nothing and protects the legacy entry
    // point from regressions on adversarial ciphertexts.
    let _ = decrypt_item(ciphertext, &nonce, &key);
});
