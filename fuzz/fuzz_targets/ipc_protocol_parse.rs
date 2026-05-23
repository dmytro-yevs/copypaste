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
    // them through `serde_json::from_slice::<Request>`. `Request` is fully
    // owned (no borrowed fields), so a plain `from_slice` works.
    let _ = serde_json::from_slice::<copypaste_ipc::Request>(data);

    // Also exercise the Response decoder — both sides of the wire share
    // the same panic-safety contract.
    //
    // `Response::error_code` is typed `Option<&'static str>` (the daemon
    // only ever sets it from `ERR_CODE_*` string literals, which already
    // have `'static` lifetime). That field's `Deserialize<'de>` impl
    // requires the input to outlive `'static`, so we cannot hand it a
    // `&[u8]` whose lifetime is bound to the fuzz iteration. The standard
    // workaround in fuzz harnesses is to leak a heap copy: the leaked
    // bytes have `'static` lifetime, which satisfies the bound. Each
    // iteration leaks ~|data| bytes — acceptable because libFuzzer caps
    // run length and the process is short-lived.
    let leaked: &'static [u8] = Box::leak(data.to_vec().into_boxed_slice());
    let _ = serde_json::from_slice::<copypaste_ipc::Response>(leaked);
});
