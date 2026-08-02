//! Getting history out of a device and back into one.
//!
//! Here rather than in the daemon because Android has no daemon: the app links
//! this crate in its own process and calls the same two functions the IPC
//! handlers call. A second export that forgot a skip count, or a second import
//! that trusted the file's `is_sensitive`, is the drift CLAUDE.md rule 1 exists
//! to stop — and the second of those is a security regression, not a cosmetic
//! one.
//!
//! # An export says what it left out
//!
//! Three counts come back beside the items, always, including when they are
//! zero. v1 shipped `skipped_non_text` for exactly this reason
//! (`CopyPaste-93yr`): a file that quietly contains fewer items than the history
//! it was taken from is worse than one that says so, because the user only
//! finds out when they need it.
//!
//! # Sensitive items are excluded by default
//!
//! An export is plaintext and leaves the app's control the moment it is written.
//! `include_sensitive` defaults to false on the wire and in every client, and a
//! withheld item is *counted*, so "why is this shorter" has an answer
//! (P2-tj9s).
//!
//! # An import is an ingest, not an insert
//!
//! Every item goes through [`crate::ingest_into`] — the same path a capture
//! takes — so the detector runs again over the plaintext and the result is
//! OR-ed with whatever the file claimed. A hand-edited export cannot
//! reintroduce a credential marked clean (manifest 04, PG-26). A malformed
//! batch is refused whole, before anything is written.

mod export;
mod import;

pub use export::export;
pub use import::{import, ImportError, MAX_IMPORT_ITEMS};

#[cfg(test)]
mod testkit;
