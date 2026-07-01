//! Shared sync pipeline helpers used by BOTH the Supabase cloud path
//! ([`crate::cloud`]) and the relay-as-database path ([`crate::relay`]).
//!
//! These functions are the platform-independent crypto + storage glue:
//!
//! - **Upload side:** `decrypt_item_plaintext` (local ciphertext → plaintext)
//!   and `wrap_and_check_cloud_upload_plaintext` (prepend the file-identity
//!   header + enforce the sync size ceiling). The caller then runs
//!   `encrypt_for_cloud(sync_key, item_id, wrapped)` to produce the SAME opaque
//!   blob for either transport.
//! - **Download side:** `build_local_item` / `build_local_blob_item`
//!   (decrypted plaintext → a locally-re-encrypted [`copypaste_core::ClipboardItem`]) and
//!   `replace_cloud_item_by_item_id` (atomic LWW in-place replace).
//! - `decode_payload_ct` decodes a PostgREST `bytea` (`\x<hex>`) or bare
//!   base64 ciphertext field.
//!
//! Extracted from `cloud.rs` so the relay client can reuse the byte-for-byte
//! identical envelope without pulling in `copypaste-supabase`. Always
//! compiled (see `lib.rs` doc comment on `pub mod sync_common;`); `cloud.rs`
//! re-imports every symbol via `use crate::sync_common::*;` so its call sites
//! and tests are unchanged.
//!
//! Split (ADR-017, CopyPaste-vp63.7) into:
//! - [`envelope`] — cloud file-identity header + `decode_payload_ct`.
//! - [`decrypt`] — local decrypt (upload side).
//! - [`rebuild`] — local rebuild (download side).
//! - [`storage`] — atomic LWW replace-by-`item_id`.
//! - [`wifi_gate`] — shared "Wi-Fi only" outbound gate.
//!
//! # Security
//! Never logs plaintext, key bytes, or ciphertext.

mod decrypt;
mod envelope;
mod rebuild;
mod storage;
mod wifi_gate;

/// Per-request HTTP timeout shared by all sync paths (cloud push/poll and
/// relay push/poll). 30 s is generous for single-row REST operations while
/// still bounding worst-case latency to a recoverable window. Without a
/// timeout, reqwest's default is infinite — one unresponsive endpoint would
/// block the whole sync loop permanently.
pub(crate) const SYNC_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) use decrypt::{decrypt_item_plaintext, decrypt_item_plaintext_blocking};
pub(crate) use envelope::{decode_payload_ct, wrap_and_check_cloud_upload_plaintext};
// `decode_cloud_file_payload` / `encode_cloud_file_payload` / the
// `CLOUD_FILE_*` constants are additionally re-exported here (beyond
// `decode_payload_ct` / `wrap_and_check_cloud_upload_plaintext` above) for
// `cloud::bytea_e2e`'s fake-PostgREST test harness, which drives the cloud
// file-identity envelope directly rather than through the normal
// wrap/decrypt pipeline. Gated to match `bytea_e2e`'s own
// `#[cfg(all(test, feature = "cloud-sync"))]` so this doesn't trip an
// unused-import warning in builds without that combination.
#[cfg(all(test, feature = "cloud-sync"))]
pub(crate) use envelope::{
    decode_cloud_file_payload, encode_cloud_file_payload, CLOUD_FILE_HEADER_VERSION,
    CLOUD_FILE_LEGACY_MIME, CLOUD_FILE_LEGACY_NAME,
};
pub(crate) use rebuild::build_local_item;
pub(crate) use storage::replace_cloud_item_by_item_id;
pub use wifi_gate::should_skip_on_cellular;
