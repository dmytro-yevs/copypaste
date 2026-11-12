//! The receiving device's decision about one incoming clip version, made in
//! the transaction that applies it, and the content rules both capture and
//! receipt evaluate.

use rusqlite::Connection;

use super::expiry::Clocks;
use super::{IncomingItem, Store, StoreError};
use crate::rules::apply::{self, CurrentRules};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiptOutcome {
    /// Write the version.
    Store,
    /// Persist nothing and report the version as stored, so the sender's
    /// cursor advances past it.
    Discard,
}

/// Decides before the write. A version already past its synced deadline is
/// discarded unseen; a tombstone, known or not, is always taken.
pub(crate) fn on_received(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<ReceiptOutcome, StoreError> {
    if !incoming.deleted && super::expiry::arrived_expired(tx, incoming, crate::now_ms())? {
        return Ok(ReceiptOutcome::Discard);
    }
    Ok(ReceiptOutcome::Store)
}

/// Runs this device's protection rules and rule tags on a live version that
/// was just written. Nothing here syncs or deletes on its own. A clip a rule
/// masks as secret leaves the search indexes, like a secret clip.
pub(crate) fn after_stored(tx: &Connection, incoming: &IncomingItem<'_>) -> Result<(), StoreError> {
    if incoming.deleted || !apply::write_receipt(tx, incoming, Clocks::now())? {
        return Ok(());
    }
    super::search::delete_fts_row_in_tx(tx, incoming.id)?;
    tx.execute(
        "UPDATE clipboard_items SET fts_rowid = NULL WHERE id = ?1",
        [incoming.id],
    )?;
    tx.execute(
        "DELETE FROM module_search_documents WHERE item_id = ?1",
        [incoming.id],
    )?;
    tx.execute(
        "DELETE FROM module_search_vectors WHERE item_id = ?1",
        [incoming.id],
    )?;
    Ok(())
}

impl Store {
    /// The content rules this device runs, compiled once per rules revision
    /// and shared by capture and receipt.
    pub fn content_rules(&self) -> Result<CurrentRules, StoreError> {
        apply::current_rules(&*self.conn()?)
    }
}
