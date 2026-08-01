//! Core: crypto, storage, and sensitive-content detection.

#![forbid(unsafe_code)]

pub mod binary;
pub mod crypto;
pub mod ingest;
pub mod sensitive;
pub mod storage;
pub mod sync;
pub mod transfer;

pub use binary::{
    item_id as binary_item_id, metadata as binary_metadata, open as open_binary,
    seal as seal_binary, BinaryMetadata, FileMetadata, CHUNK_BYTES,
};
pub use crypto::{decrypt, encrypt, CryptoError, ItemKey, Keyring};
pub use ingest::{
    ingest, ingest_binary_into_with_capture_context, ingest_into, ingest_into_with_capture_context,
    IngestError, Ingested,
};
pub use sensitive::{
    purge_indexed_secrets, sweep_sensitive, Detector, Finding, PurgeReport, Severity,
    DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED,
};
pub use storage::{
    compute_content_hash, is_v1_database, origin_or, v1_database_in, verify_integrity,
    verify_schema, DeviceIdentity, IncomingItem, IndexedText, Ingest, ItemCursor, NewItem, Page,
    Store, StoreError, StoredItem, Version, V1_DATABASE_FILENAME,
};
pub use sync::{MergeError, RemoteVersion, StoreSource};
pub use transfer::{export, import, ImportError, MAX_IMPORT_ITEMS};

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
