//! Fuzz target: arbitrary bytes → `copypaste_ipc::Request` deserialization.
//!
//! Goal: the JSON wire decoder MUST NOT panic on malformed input. Any panic
//! constitutes a denial-of-service vector for the daemon (a single bad
//! frame from a UI/CLI client would abort the daemon process).
//!
//! Returning `Err(serde_json::Error)` is the expected outcome for invalid
//! input; this target only fails when the call panics or aborts.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Primary path: the daemon receives raw bytes from the UDS and feeds
    // them through `serde_json::from_slice::<Request>`.
    let _ = serde_json::from_slice::<copypaste_ipc::Request>(data);

    // Also exercise the Response decoder — both sides of the wire share
    // the same panic-safety contract.
    let _ = serde_json::from_slice::<copypaste_ipc::Response>(data);
});
