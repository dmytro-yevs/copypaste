//! Fuzz target: snapshot / import wire-format decoder.
//!
//! The CLI `import` command consumes a JSON dump of clipboard items
//! produced by a previous `export` run. Each entry is shipped to the
//! daemon inside `Request::params.items` as opaque JSON; the daemon's
//! `import` handler decodes the per-item fields out of `serde_json::Value`
//! one-by-one (see `crates/copypaste-daemon/src/ipc.rs` — the "import"
//! arm). A panic in `serde_json` here lets a hand-crafted dump file abort
//! the CLI (or, worse, the daemon if the same payload reaches it over
//! the UDS).
//!
//! ## Invariant
//!
//! `serde_json::from_slice` MUST NOT panic on arbitrary bytes — malformed
//! input is expected to surface as `Err(serde_json::Error)`.
//!
//! ## Surface fuzzed
//!
//! 1. A single import row decoded as `serde_json::Value` (one entry of
//!    the snapshot — matches the daemon's per-item decode loop).
//! 2. A full snapshot decoded as `Vec<serde_json::Value>` (the whole
//!    dump as it appears on disk before being wrapped in `params.items`).
//! 3. Embedded inside a `Request` envelope — mirrors the actual
//!    transport path from CLI → daemon when `import` ships the rows
//!    over the UDS.
//!
//! ## Note on `ImportItem`
//!
//! An earlier iteration of this target imported
//! `copypaste_ipc::types::ImportItem`, but that struct was never landed
//! and the placeholder `types` module has since been removed. The real
//! wire surface for imported items is untyped `serde_json::Value`, which
//! the daemon's `import` handler walks field-by-field. Fuzzing `Value`
//! therefore matches the actual deserialization performed in production.

#![no_main]

use copypaste_ipc::Request;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Path 1: single row, as parsed by the daemon's per-item decode loop.
    let _ = serde_json::from_slice::<serde_json::Value>(data);

    // Path 2: full snapshot (array of rows) — the on-disk dump shape.
    let _ = serde_json::from_slice::<Vec<serde_json::Value>>(data);

    // Path 3: snapshot embedded as `Request::params`. The daemon's IPC
    // dispatcher pulls rows out of `params.items` before walking each
    // entry, so panic-safety must hold for the wrapping envelope as well.
    let _ = serde_json::from_slice::<Request>(data);
});
