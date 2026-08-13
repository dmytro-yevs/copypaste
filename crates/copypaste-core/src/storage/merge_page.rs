//! Atomic read-then-write for batch merge, inside one IMMEDIATE transaction.

use std::collections::HashMap;

use super::connection::write_tx;
use super::model::StoreError;
use super::store::Store;
use super::versions::{upsert_in_tx, version_summaries_on, IncomingItem, Version};

/// The snapshot the caller planned from has changed under the write lock.
///
/// Crate-private with [`Store::merge_page`]: it is how the batch merge holds
/// its transaction, not something a caller outside the merge should be handed.
pub(crate) enum MergePageError {
    Store(StoreError),
    /// The authoritative summaries (read inside the IMMEDIATE tx) differ
    /// from the expected ones. The caller must re-prepare its decisions
    /// against this snapshot and retry.
    SnapshotChanged(HashMap<String, Version>),
}

impl From<StoreError> for MergePageError {
    fn from(e: StoreError) -> Self {
        MergePageError::Store(e)
    }
}

impl From<rusqlite::Error> for MergePageError {
    fn from(e: rusqlite::Error) -> Self {
        MergePageError::Store(e.into())
    }
}

impl Store {
    /// Read summaries and upsert in one IMMEDIATE transaction, so the
    /// decision and the write see the same snapshot (B6).
    ///
    /// `expected` is the summary snapshot the caller used to build its
    /// decisions. If the authoritative read (inside the write lock) differs
    /// from `expected`, the transaction is rolled back and the authoritative
    /// summaries are returned in the `Err` variant so the caller can
    /// re-prepare and retry.
    ///
    /// Each item gets its own savepoint, so the dedup index refusing one row
    /// drops that row and not the page. Answering for it is the caller's:
    /// see [`crate::sync`]'s batch merge.
    pub(crate) fn merge_page(
        &self,
        ids: &[&str],
        expected: &HashMap<String, Version>,
        writes: &[IncomingItem<'_>],
    ) -> Result<Vec<bool>, MergePageError> {
        let mut conn = self.conn()?;
        let mut tx = write_tx(&mut conn)?;
        let summaries = version_summaries_on(&tx, ids)?;
        if summaries != *expected {
            return Err(MergePageError::SnapshotChanged(summaries));
        }
        let mut stored = Vec::with_capacity(writes.len());
        for item in writes {
            let mut savepoint = tx.savepoint()?;
            let written = upsert_in_tx(&savepoint, item)?;
            if written {
                savepoint.commit()?;
            } else {
                savepoint.rollback()?;
            }
            stored.push(written);
        }
        tx.commit()?;
        Ok(stored)
    }
}
