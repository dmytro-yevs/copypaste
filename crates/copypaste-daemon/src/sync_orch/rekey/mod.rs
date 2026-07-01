//! Cross-device content-key re-keying for the sync orchestrator (P2P Phase 3).
//!
//! Split (ADR-017, CopyPaste-vp63.9) from the former flat `rekey.rs` into:
//! - [`crypto_ctx`] — [`SyncCrypto`] (per-peer sync-key cache) + [`AutoApplyCtx`].
//! - [`outbound`] — decrypt at-rest ciphertext under the local key, re-encrypt
//!   under a shared/pairwise sync key (text + image/file blob).
//! - [`inbound`] — decrypt a sync-key-wrapped incoming item, re-encrypt (text)
//!   or re-chunk (image/file) under THIS device's local key.
//!
//! This module is a thin facade: it re-exports the exact same public surface
//! the flat file exposed so `sync_orch/mod.rs`'s outer `pub use rekey::{...}`
//! line — and every `super::rekey::{...}` import in `merge.rs`/`catchup.rs`/
//! the test module — keep compiling unchanged.

mod crypto_ctx;
mod inbound;
mod outbound;

pub use crypto_ctx::{AutoApplyCtx, SyncCrypto};
pub use outbound::{rekey_outbound_for_peer, RekeyOutcome, SYNC_MAX_BLOB_BYTES};

// `pub(super)` here means "visible to `sync_orch`" (the parent of this
// `rekey` module) — re-establishing the exact reach the flat `rekey.rs` file
// had, now that these items live one directory level deeper. Only
// `rekey_inbound` is actually consumed from outside `rekey` (by
// `sync_orch::merge`); `parse_file_name_mime` / `read_png_dimensions` /
// `rewrap_inbound_blob` are internal to `inbound.rs` (or reached directly by
// `outbound.rs` via `super::inbound::...`), so they are NOT re-exported here.
pub(super) use inbound::rekey_inbound;
// `rekey_outbound` (bare) is only reached by `rekey`'s own test submodules
// (`tests.rs` / `blob_tests.rs`, both descendants of this module, both
// `#[cfg(test)]`) via `use super::*;` — a private `use` (no `pub` qualifier)
// is sufficient for that reach and, unlike `pub(super)`, does not exceed
// `rekey_outbound`'s actual `pub(super)` (i.e.
// visible-to-`rekey`-and-descendants) visibility in `outbound.rs`.
// `recover_blob_plaintext` / `rekey_blob_outbound` /
// `rekey_blob_outbound_with_key` / `rekey_outbound_text_with_key` are used
// only inside `outbound.rs` itself, so they are NOT re-exported here. Gated
// to match its only consumers and avoid an unused-import warning in
// non-test builds.
#[cfg(test)]
use outbound::rekey_outbound;

// Relocated (ADR-017, CopyPaste-vp63.3) from the former flat `sync_orch/mod.rs`
// test module: text round-trip + per-peer isolation (`tests.rs`), image/file
// blob round-trip (`blob_tests.rs`).
#[cfg(test)]
mod blob_tests;
#[cfg(test)]
mod tests;
