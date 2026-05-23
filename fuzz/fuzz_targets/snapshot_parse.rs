//! Fuzz target: snapshot / import wire-format decoder.
//!
//! The CLI `import` command consumes a JSON dump of clipboard items
//! produced by a previous `export` run. Each entry deserialises into
//! [`copypaste_ipc::types::ImportItem`] and is then handed to the daemon
//! for re-insertion. A panic in `serde_json` here lets a hand-crafted dump
//! file abort the CLI (or, worse, the daemon if the same payload reaches
//! it over the UDS).
//!
//! ## Invariant
//!
//! `serde_json::from_slice` MUST NOT panic on arbitrary bytes — malformed
//! input is expected to surface as `Err(serde_json::Error)`.
//!
//! ## Surface fuzzed
//!
//! 1. A single `ImportItem` (one row of the snapshot).
//! 2. A `Vec<ImportItem>` (the whole snapshot dump as it appears on disk).
//! 3. Embedded inside a `Request::params` envelope — mirrors the actual
//!    transport path from CLI → daemon when `import` ships the rows over
//!    the UDS rather than reading them server-side.

#![no_main]

use copypaste_ipc::{types::ImportItem, Request};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Path 1: single row.
    let _ = serde_json::from_slice::<ImportItem>(data);

    // Path 2: full snapshot (array of rows).
    let _ = serde_json::from_slice::<Vec<ImportItem>>(data);

    // Path 3: snapshot embedded as `Request::params`. The daemon's IPC
    // dispatcher pulls rows out of `params` before deserialising into
    // `ImportItem`, so panic-safety must hold for the wrapping envelope
    // as well as the inner type.
    let _ = serde_json::from_slice::<Request>(data);
});
