//! Getting history out of a device and back into one.
//!
//! Here rather than in the daemon because Android has no daemon: the app links
//! this crate in its own process and calls the same two functions the IPC
//! handlers call. A second export that forgot a skip count would drift from the
//! product contract.
//!
//! # An export says what it left out
//!
//! Three counts come back beside the items, always, including when they are
//! zero. `CopyPaste-93yr`: a file that quietly contains fewer items than the history
//! it was taken from is worse than one that says so, because the user only
//! finds out when they need it.
//!
//! # An import is an ingest, not an insert
//!
//! Every item goes through [`crate::ingest_into`] — the same path a capture
//! takes. A malformed batch is refused whole, before anything is written.

mod export;
mod import;

pub use export::{export, ExportError};
pub use import::{import, import_with_current_retention, ImportError, MAX_IMPORT_ITEMS};

#[cfg(test)]
mod testkit;
