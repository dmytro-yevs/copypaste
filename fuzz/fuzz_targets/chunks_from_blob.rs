//! Fuzz target: `copypaste_core::chunks_from_blob` against arbitrary bytes.
//!
//! ## Invariant
//!
//! `chunks_from_blob` parses a length-prefixed wire format produced by
//! `chunks_to_blob`:
//!
//! ```text
//!   count:    u32 BE
//!   for _ in 0..count:
//!     wire_len: u32 BE
//!     wire:     [u8; wire_len]
//!       version:  u8
//!       index:    u32 BE
//!       is_final: u8
//!       nonce:    [u8; 24]
//!       len:      u32 BE
//!       ct:       [u8; len]
//! ```
//!
//! Every length prefix is attacker-controlled and historically a classic
//! source of panics: arithmetic overflow on `pos + wire_len`, slice indexing
//! past `blob.len()`, `Vec::with_capacity(count as usize)` on an absurd
//! count, etc. The function MUST surface every malformed input as
//! `Err(ImageError::Decode(_))`, never panic, abort, or hang.
//!
//! ## Threat model
//!
//! Image rows are persisted as opaque BLOBs in SQLite. A tampered local DB
//! row OR an inbound P2P-synced chunk-set could carry adversarial bytes;
//! `chunks_from_blob` is the only path back to typed `EncryptedChunk`s, so
//! a panic here turns into a daemon crash on every read of the row.

#![no_main]

use copypaste_core::chunks_from_blob;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The function returns a Result; fuzzer-generated input will almost
    // always be `Err(_)`. The contract under test is "never panics on any
    // byte sequence" — discarding both arms is the correct behaviour.
    let _ = chunks_from_blob(data);
});
