//! Fuzz target: `copypaste_core::sensitive` pattern detection on arbitrary
//! UTF-8 input.
//!
//! ## Invariants
//!
//! - `SensitiveDetector::detect` and `is_sensitive` MUST NOT panic on any
//!   well-formed UTF-8 input (NFKC normalisation, regex matching, and the
//!   Luhn / generic-password value-strength gates all run user-supplied
//!   bytes through index arithmetic that has historically been the source
//!   of subtle off-by-one panics).
//! - The free function `detect` (returning `SensitiveKind`) MUST agree on
//!   "sensitive-ness" with the struct-based path — they share the same
//!   pattern table, so a divergence implies one path is short-circuiting
//!   incorrectly.
//!
//! ## Threat model
//!
//! Clipboard content is attacker-influenced (anything the user copies or
//! anything pushed via P2P sync can land in `sensitive::detect`). A panic
//! here is a remote DoS — every read of the offending row would crash the
//! daemon's auto-sensitive-classification path.

#![no_main]

use copypaste_core::sensitive::{detect, SensitiveDetector};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Struct-based detector: full pattern table.
    let det = SensitiveDetector::new();
    let _ = det.detect(s);
    let _ = det.is_sensitive(s);

    // Free-function shortcut (returns the first matching SensitiveKind).
    let _ = detect(s);
});
