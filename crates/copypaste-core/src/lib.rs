//! Core: crypto, storage, and sensitive-content detection.

// `forbid` everywhere it can be kept, which is every target but one. Windows
// has no safe path to DPAPI: the wrappers on crates.io free the unsealed buffer
// without wiping it, which loses I-12 on the device secret itself. `deny` is
// the same error with one auditable exception — `crypto::keystore::windows`,
// which carries `#![allow(unsafe_code)]` and nothing else in the tree does.
#![cfg_attr(not(target_os = "windows"), forbid(unsafe_code))]
#![cfg_attr(target_os = "windows", deny(unsafe_code))]

pub mod binary;
pub mod crypto;
pub mod image_preview;
pub mod ingest;
pub mod p2p_contract;
pub mod retention;
pub mod sensitive;
pub mod storage;
pub mod sync;
pub mod transfer;

pub use binary::{
    item_id as binary_item_id, metadata as binary_metadata, open as open_binary,
    seal as seal_binary, BinaryMetadata, FileMetadata, CHUNK_BYTES,
};
pub use crypto::{decrypt, encrypt, CryptoError, ItemKey, Keyring};
pub use image_preview::{thumbnail_png, ImagePreviewError, ImageThumbnail, MAX_THUMBNAIL_EDGE};
pub use ingest::{
    ingest, ingest_binary_into_with_capture_context, ingest_binary_into_with_capture_source,
    ingest_into, ingest_into_with_capture_context, ingest_into_with_capture_source, IngestError,
    Ingested,
};
pub use sensitive::{
    purge_indexed_secrets, purge_indexed_secrets_in_transaction, sweep_sensitive, Detector,
    Finding, PurgeReport, Severity, DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED,
};
pub use storage::{
    compute_content_hash, origin_or, verify_integrity, verify_schema, DeviceIdentity, IncomingItem,
    IndexedText, Ingest, ItemCursor, NewItem, Page, RestoreError, Store, StoreError, StoredItem,
    Version,
};
pub use sync::{local_winner_stamp, MergeError, OpenVersionError, RemoteVersion, StoreSource};
pub use transfer::{export, import, ImportError, MAX_IMPORT_ITEMS};

/// Milliseconds since the Unix epoch.
///
/// One helper, called everywhere. v1 had several time sources and a
/// clock-skew bug that came from mixing them.
pub use copypaste_clock::now_ms;
