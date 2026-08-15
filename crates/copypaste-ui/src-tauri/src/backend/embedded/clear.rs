//! Clearing history, and naming the set to clear.
//!
//! Separated from the command surface in `super` because the two belong
//! together and to nothing else: the ceiling is only meaningful as the argument
//! a later `clear` is given.

use super::{EmbeddedBackend, Result};
use crate::backend::BackendError;

/// The set of rows the user is looking at, as a bound a later [`clear`] can be
/// handed.
///
/// A `rowid`, so it does not move when a clock does. The frontend defers a
/// clear behind an undo window and takes this when the user asks, so a clip
/// captured during the window is above the bound and survives.
pub(super) async fn ceiling(backend: &EmbeddedBackend) -> Result<u64> {
    backend
        .blocking(move |inner| {
            inner
                .state
                .store
                .max_rowid()
                .map(|ceiling| ceiling.max(0) as u64)
                .map_err(|_| BackendError::internal("history could not be read"))
        })
        .await
}

/// Tombstone every live, unpinned row at or below `through`, or all of them
/// when it is `None`.
pub(super) async fn clear(backend: &EmbeddedBackend, through: Option<i64>) -> Result<u64> {
    backend
        .blocking(move |inner| {
            let mutation_started = copypaste_core::now_ms();
            let removed = inner
                .state
                .store
                .delete_all_through(through.unwrap_or(i64::MAX))
                .map_err(|_| BackendError::internal("history could not be cleared"))?;
            if removed > 0 {
                inner.note_version_written(mutation_started);
                inner.note_local_version(mutation_started);
                inner.publish_items(false, 0);
            }
            Ok(removed)
        })
        .await
}
