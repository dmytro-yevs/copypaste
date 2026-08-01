//! Core: crypto, storage, and sensitive-content detection.

#![forbid(unsafe_code)]

pub mod binary;
pub mod crypto;
pub mod image_preview;
pub mod ingest;
pub mod sensitive;
pub mod storage;
pub mod sync;
pub mod transfer;

pub use binary::{
    BinaryMetadata, CHUNK_BYTES, FileMetadata, item_id as binary_item_id,
    metadata as binary_metadata, open as open_binary, seal as seal_binary,
};
pub use crypto::{CryptoError, ItemKey, Keyring, decrypt, encrypt};
pub use image_preview::{ImagePreviewError, ImageThumbnail, MAX_THUMBNAIL_EDGE, thumbnail_png};
pub use ingest::{
    IngestError, Ingested, ingest, ingest_binary_into_with_capture_context,
    ingest_binary_into_with_capture_source, ingest_into, ingest_into_with_capture_context,
    ingest_into_with_capture_source,
};
pub use sensitive::{
    DEFAULT_SENSITIVE_TTL, Detector, Finding, PurgeReport, SENSITIVE_TTL_DISABLED, Severity,
    purge_indexed_secrets, purge_indexed_secrets_in_transaction, sweep_sensitive,
};
pub use storage::{
    DeviceIdentity, IncomingItem, IndexedText, Ingest, ItemCursor, NewItem, Page, Store,
    StoreError, StoredItem, Version, compute_content_hash, origin_or, verify_integrity,
    verify_schema,
};
pub use sync::{MergeError, RemoteVersion, StoreSource};
pub use transfer::{ImportError, MAX_IMPORT_ITEMS, export, import};

/// Milliseconds since the Unix epoch.
///
/// One helper, called everywhere. v1 had several time sources and a
/// clock-skew bug that came from mixing them.
pub fn now_ms() -> i64 {
    use copypaste_clock::{SystemWallClock, WallClock};
    SystemWallClock.now_ms()
}
