//! Core: crypto and storage.

// `forbid` everywhere it can be kept, which is every target but one. Windows
// has no safe path to DPAPI: the wrappers on crates.io free the unsealed buffer
// without wiping it, which loses I-12 on the device secret itself. `deny` is
// the same error with one auditable exception — `crypto::keystore::windows`,
// which carries `#![allow(unsafe_code)]` and nothing else in the tree does.
#![cfg_attr(not(target_os = "windows"), forbid(unsafe_code))]
#![cfg_attr(target_os = "windows", deny(unsafe_code))]

pub mod binary;
pub mod clipboard_payload;
pub mod crypto;
pub mod device_name;
pub mod image_preview;
pub mod ingest;
pub mod p2p_contract;
pub mod retention;
pub mod semantic;
pub mod settings_record;
pub mod storage;
pub mod sync;
pub mod transfer;

pub use binary::{
    item_id as binary_item_id, metadata as binary_metadata, open as open_binary,
    seal as seal_binary, BinaryMetadata, FileMetadata, PayloadMetadata, SourceAppIconMetadata,
    CHUNK_BYTES,
};
pub use clipboard_payload::{ClipboardPayload, ClipboardWriteError};
pub use crypto::{decrypt, encrypt, CryptoError, ItemKey, Keyring};
pub use image_preview::{
    image_metadata, thumbnail_png, ImageMetadata, ImagePreviewError, ImageThumbnail,
    MAX_THUMBNAIL_EDGE,
};
pub use ingest::{
    ingest, ingest_binary_into_with_capture_context, ingest_binary_into_with_capture_source,
    ingest_binary_into_with_capture_source_metadata, ingest_into, ingest_into_with_capture_context,
    ingest_into_with_capture_source, ingest_into_with_capture_source_metadata,
    ingest_into_with_capture_source_metadata_with_current_retention,
    ingest_into_with_capture_source_with_current_retention, IngestError, Ingested,
};
pub use semantic::{classify_semantic, SemanticClassification};
pub use storage::{
    compute_content_hash, origin_or, verify_integrity, verify_schema, DeviceIdentity,
    HistoryCursor, HistoryDeviceFacet, HistoryFacets, HistoryPage, HistoryQuery, HistorySort,
    HistorySourceAppFacet, IncomingItem, Ingest, ItemCursor, NewItem, Page, RestoreError, Store,
    StoreError, StoredItem, Version,
};
pub use sync::{local_winner_stamp, MergeError, OpenVersionError, RemoteVersion, StoreSource};
pub use transfer::{export, import, ImportError, MAX_IMPORT_ITEMS};

/// Milliseconds since the Unix epoch.
///
/// One helper, called everywhere; mixing time sources breaks ordering under
/// clock skew.
pub use copypaste_clock::now_ms;
