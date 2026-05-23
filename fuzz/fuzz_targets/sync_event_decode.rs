//! Fuzz target: arbitrary bytes → `copypaste_sync::protocol::Message::decode`.
//!
//! Goal: the P2P sync wire decoder MUST NOT panic on bytes received from
//! a remote peer. A peer is by definition untrusted input — a panic here
//! is a remote-DoS against the daemon.
//!
//! `Err(serde_json::Error)` is the expected outcome for invalid input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Sync engine strips the 4-byte length prefix before calling decode,
    // so we fuzz the raw JSON-decode path the same way.
    let _ = copypaste_sync::protocol::Message::decode(data);
});
