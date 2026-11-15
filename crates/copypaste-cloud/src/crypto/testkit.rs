//! Fixtures shared by this module's tests.

use super::key::{derive_sync_key, SyncKey, KEY_LEN};

// Twelve characters exactly, so the boundary is exercised by the happy path
// as well as by the rejection tests.
pub(super) const PASS: &str = "correct horse battery staple";
pub(super) const ACCOUNT: &str = "3f2b1c0a-0000-4000-8000-000000000001";
pub(super) const ITEM: &str = "1f3c9a4e-0000-4000-8000-000000000001";

/// The fixture key.
///
/// Derived once for the whole binary and rebuilt from bytes thereafter:
/// Argon2id is deliberately expensive, and paying 19 MiB per fixture would
/// make the suite slow enough that people stop running it. The tests that
/// are *about* derivation call [`derive_sync_key`] directly.
pub(super) fn key() -> SyncKey {
    static KEY: std::sync::OnceLock<[u8; KEY_LEN]> = std::sync::OnceLock::new();
    SyncKey::from_bytes(*KEY.get_or_init(|| *derive_sync_key(PASS, ACCOUNT).unwrap().to_bytes()))
}
