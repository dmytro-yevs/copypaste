//! The sync view of the item table, plus this device's identity.
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
//! # Layout
//!
//! Split on the direction data moves, because that is the direction the rules
//! run in. This file holds the three shapes that cross the boundary; the
//! submodules hold the operations:
//!
//! * [`open`] — the second connection, the key check that fails closed, and
//!   this device's identity.
//! * [`read`] — what a session may advertise, compare against, and serve. The
//!   `is_sensitive = 0` filter that decides what leaves the device lives here.
//! * [`write`] — `record_origin` and `apply`: the LWW write, the pin that stays
//!   local, and the unconditional FTS delete that keeps a sensitive or deleted
//!   version out of the index.
//! * [`error`] — the closed error set, plus the two `rusqlite` code tests that
//!   distinguish "wrong key" and "dedup collision" from a real failure.
//!
//! When the four `Store` additions above land, `read` and `write` are the two
//! files that disappear.

mod error;
mod open;
mod read;
mod write;

#[cfg(test)]
mod testutil;

pub use error::MetaError;
pub use open::Meta;

use copypaste_p2p::protocol::ItemSummary;

/// One item as the merge sees it locally.
#[derive(Debug, Clone)]
pub struct LocalVersion {
    pub summary: ItemSummary,
    /// Never empty — `SyncMessage::validate` rejects an empty id, and an item
    /// with no recorded origin is one this device captured.
    pub origin_device_id: String,
    pub is_sensitive: bool,
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
