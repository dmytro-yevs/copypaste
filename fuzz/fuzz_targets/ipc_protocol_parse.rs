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
    // only ever sets it from `ERR_CODE_*` string literals). serde's
    // `Deserialize<'de>` for that field requires the input slice to
    // outlive `'static`, which a fuzz `&[u8]` cannot. Promote the slice
    // to `'static` via a heap allocation, run the parse, then drop the
    // allocation — LeakSanitizer in libFuzzer would otherwise fail the
    // run. Because the field type is `&'static str`, serde never actually
    // borrows from the input on success (an arbitrary string slice can't
    // satisfy `'static`, so the only valid decode is `None`/absent);
    // therefore the box is safe to free once `from_slice` returns.
    let raw: *mut [u8] = Box::into_raw(data.to_vec().into_boxed_slice());
    // SAFETY: `raw` was just produced from `Box::into_raw` and is not
    // aliased; the reference's lifetime ends before we reclaim the box.
    let _ = serde_json::from_slice::<copypaste_ipc::Response>(unsafe { &*raw });
    // SAFETY: reclaiming the same allocation we leaked above.
    unsafe { drop(Box::from_raw(raw)) };
});
