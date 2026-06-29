//! #12 — TS size consts parity: Rust guard.
//!
//! Source of truth for the Rust values:
//!   `crates/copypaste-core/src/config/defaults.rs`
//!   (re-exported via `crates/copypaste-core/src/config/mod.rs` → `pub use defaults::*`)
//!
//! Source of truth for the TS values:
//!   `crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts`
//!
//! The TS file declares:
//!   DEFAULT_MAX_TEXT_BYTES       = 10 * 1024 * 1024          (10 MiB)
//!   DEFAULT_MAX_IMAGE_BYTES      = 64 * 1024 * 1024          (64 MiB)
//!   DEFAULT_MAX_FILE_BYTES       = 100 * 1024 * 1024         (100 MiB)
//!   DEFAULT_STORAGE_QUOTA_BYTES  = 10 * 1024 * 1024 * 1024   (10 GiB)
//!
//! If this test fails after changing a Rust default, update the corresponding
//! literal in `settingsSliders.ts` to keep UI sliders in sync.
//!
//! CROSS-LINK: the companion TS test lives at
//!   `crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.parity.test.ts`

use copypaste_core::config::MAX_FILE_SIZE_BYTES;
use copypaste_core::config::MAX_IMAGE_SIZE_BYTES;
use copypaste_core::config::MAX_TEXT_SIZE_BYTES;
use copypaste_core::config::STORAGE_QUOTA_BYTES;

/// Literals copied verbatim from settingsSliders.ts — these are the EXPECTED values.
/// Update both here AND in the TS file when a Rust default changes.
///
/// Source: crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts
///   DEFAULT_MAX_TEXT_BYTES       = 10 * 1024 * 1024
///   DEFAULT_MAX_IMAGE_BYTES      = 64 * 1024 * 1024
///   DEFAULT_MAX_FILE_BYTES       = 100 * 1024 * 1024
///   DEFAULT_STORAGE_QUOTA_BYTES  = 10 * 1024 * 1024 * 1024
const TS_DEFAULT_MAX_TEXT_BYTES: u64 = 10 * 1024 * 1024;
const TS_DEFAULT_MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
const TS_DEFAULT_MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;
const TS_DEFAULT_STORAGE_QUOTA_BYTES: u64 = 10 * 1024 * 1024 * 1024;

#[test]
fn max_text_size_bytes_matches_ts_default() {
    assert_eq!(
        MAX_TEXT_SIZE_BYTES, TS_DEFAULT_MAX_TEXT_BYTES,
        "Rust MAX_TEXT_SIZE_BYTES changed — update DEFAULT_MAX_TEXT_BYTES in \
         crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts"
    );
}

#[test]
fn max_image_size_bytes_matches_ts_default() {
    assert_eq!(
        MAX_IMAGE_SIZE_BYTES, TS_DEFAULT_MAX_IMAGE_BYTES,
        "Rust MAX_IMAGE_SIZE_BYTES changed — update DEFAULT_MAX_IMAGE_BYTES in \
         crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts"
    );
}

#[test]
fn max_file_size_bytes_matches_ts_default() {
    assert_eq!(
        MAX_FILE_SIZE_BYTES, TS_DEFAULT_MAX_FILE_BYTES,
        "Rust MAX_FILE_SIZE_BYTES changed — update DEFAULT_MAX_FILE_BYTES in \
         crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts"
    );
}

#[test]
fn storage_quota_bytes_matches_ts_default() {
    assert_eq!(
        STORAGE_QUOTA_BYTES, TS_DEFAULT_STORAGE_QUOTA_BYTES,
        "Rust STORAGE_QUOTA_BYTES changed — update DEFAULT_STORAGE_QUOTA_BYTES in \
         crates/copypaste-ui/src/views/SettingsView/lib/settingsSliders.ts"
    );
}
