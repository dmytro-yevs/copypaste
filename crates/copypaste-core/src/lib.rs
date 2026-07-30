//! Core: crypto, storage, and sensitive-content detection.

#![forbid(unsafe_code)]

pub mod crypto;
pub mod sensitive;
pub mod storage;

pub use crypto::{decrypt, encrypt, CryptoError, ItemKey, Keyring};
pub use sensitive::{Detector, Finding, Severity};
pub use storage::{NewItem, StoredItem, Store, StoreError};

/// Milliseconds since the Unix epoch.
///
/// One helper, called everywhere. v1 had several time sources and a
/// clock-skew bug that came from mixing them.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
