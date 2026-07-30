//! Core: crypto, storage, and sensitive-content detection.

#![forbid(unsafe_code)]

pub mod crypto;
pub mod ingest;
pub mod sensitive;
pub mod storage;

pub use crypto::{decrypt, encrypt, CryptoError, ItemKey, Keyring};
pub use ingest::{ingest, ingest_into, IngestError, Ingested};
pub use sensitive::{
    purge_indexed_secrets, sweep_sensitive, Detector, Finding, PurgeReport, Severity,
    DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED,
};
pub use storage::{
    compute_content_hash, is_v1_database, v1_database_in, IndexedText, Ingest, ItemCursor, NewItem,
    Page, Store, StoreError, StoredItem, V1_DATABASE_FILENAME,
};

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
