//! Transport-neutral history, cursor and merge contracts for sync providers.
#![forbid(unsafe_code)]
pub mod cadence;
pub mod host;
pub mod outcome;
pub mod scan;
pub mod source;
#[cfg(feature = "storage")]
pub mod store;
pub mod unreadable;
pub use outcome::{SyncError, SyncStats};
pub use source::{Applied, CloudSource, LocalItem};

/// A transport-independent clipboard egress limit, measured before encryption.
#[must_use]
pub fn too_large_to_sync(_content_type: &str, byte_len: usize) -> bool {
    byte_len > copypaste_ipc::MAX_CONTENT_BYTES
}
