//! The sync view of the item table, plus this device's identity.
//!
//! **One view, both transports.** The peer sessions in [`crate::p2p`] and the
//! cloud rounds in [`crate::cloud`] read and write the same rows through this
//! module, share the device id it mints, and keep their cursors in the same
//! `sync_device_state` table ([`state`]). That is why it sits at the crate root
//! rather than under either transport: manifest 05 INV-C2 is a bug that came
//! from two transports holding two views of one history.
//!
//! # Why this module opens the database itself
//!
//! A sync session needs five things about an item: its id, its version stamp,
//! its content hash, whether it is a tombstone, and which device first captured
//! it. `copypaste_core::StoredItem` carries the first two. The next two are
//! *columns that already exist* in `clipboard_items` (`content_hash`,
//! `deleted`) but are not projected into `StoredItem`, and the last has no
//! column at all.
//!
//! There were two ways to close that gap without touching `copypaste-core`:
//!
//! 1. Shadow the missing fields in a second store the daemon owns, writing
//!    every hash, every tombstone and every timestamp twice.
//! 2. Read the columns that are already there, on a second connection to the
//!    same SQLCipher file, and add one table for the one genuinely new fact.
//!
//! The first is the failure mode `CLAUDE.md` rule 1 is written about: two
//! implementations of "what is in this device's history", which drift the first
//! time an eviction or a delete lands on one and not the other. So this module
//! does the second. It owns exactly one new table — `sync_item_origin` — and
//! otherwise only reads and updates rows the store already manages.
//!
//! **This is a layering compromise, and it should be repaid.** The clean fix is
//! four additions to `copypaste-core::Store`, at which point every raw statement
//! below deletes itself:
//!
//! * `content_hash` and `deleted` on `StoredItem`,
//! * `Store::summaries()` — id, stamp, hash, tombstone flag, live and deleted,
//! * `Store::upsert(NewItem, deleted: bool)` — the LWW write, which is the one
//!   thing the insert-only API genuinely cannot express,
//! * an `origin_device_id` column, so this module keeps only the device row.
//!
//! When those additions land, [`read`] and [`write`] are the two files that
//! disappear. The split between them is the direction data moves, because that
//! is the direction the rules run in: the `is_sensitive = 0` filter deciding
//! what may leave the device is in [`read`], and the unconditional FTS delete
//! keeping a sensitive or deleted version out of the index is in [`write`].

mod error;
mod open;
mod read;
mod state;
mod write;

#[cfg(test)]
mod testutil;

pub use error::MetaError;
pub use open::Meta;

use copypaste_p2p::protocol::ItemSummary;

/// Run a short blocking database call without stalling the reactor.
///
/// Every operation in this module is SQLite plus an AEAD, and both transports
/// call it from inside an async task, so the work happens on whatever thread is
/// driving that task. On the daemon's multi-threaded runtime `block_in_place`
/// hands the other tasks to a different worker for the duration; on a
/// current-thread runtime — which is what `#[tokio::test]` builds — it would
/// panic, so there the call simply runs inline.
pub fn blocking<T>(f: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

/// One item as the merge sees it locally.
#[derive(Debug, Clone)]
pub struct LocalVersion {
    pub summary: ItemSummary,
    /// Never empty — `SyncMessage::validate` rejects an empty id, and an item
    /// with no recorded origin is one this device captured.
    pub origin_device_id: String,
    pub is_sensitive: bool,
    /// Local-only state, and the reason [`crate::merge`] can refuse an
    /// incoming delete. Never travels on either transport.
    pub pinned: bool,
}

/// One item on its way out to a peer, still encrypted.
#[derive(Debug, Clone)]
pub struct StoredVersion {
    pub item_id: String,
    /// `None` on a tombstone: the soft delete wiped the payload.
    pub content_ciphertext: Option<Vec<u8>>,
    pub nonce: Option<Vec<u8>>,
    pub content_type: String,
    pub content_hash: String,
    pub created_at: i64,
    pub deleted: bool,
    pub origin_device_id: String,
}

/// One item on its way in from a peer, already sealed under the local key.
pub struct IncomingVersion<'a> {
    pub item_id: &'a str,
    pub content_ciphertext: Option<&'a [u8]>,
    pub nonce: Option<&'a [u8]>,
    pub content_type: &'a str,
    pub content_hash: &'a str,
    pub created_at: i64,
    pub deleted: bool,
    pub is_sensitive: bool,
    pub origin_device_id: &'a str,
    /// Plaintext for the search index. Ignored when the item is sensitive or a
    /// tombstone — the write-time layer of "sensitive items are never indexed".
    pub search_text: Option<&'a str>,
}
