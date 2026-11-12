//! Clip expiry deadlines derived on capture and on receipt.

use rusqlite::Connection;

use super::{IncomingItem, StoreError};

/// The local deadline a received version gets from the synced rules, if any.
#[allow(dead_code)]
pub(crate) fn receipt_deadline(
    _tx: &Connection,
    _incoming: &IncomingItem<'_>,
    _now_ms: i64,
) -> Result<Option<i64>, StoreError> {
    Ok(None)
}

/// Whether a received version is already past its deadline and must be
/// discarded without persisting its plaintext.
pub(crate) fn arrived_expired(
    _tx: &Connection,
    _incoming: &IncomingItem<'_>,
    _now_ms: i64,
) -> Result<bool, StoreError> {
    Ok(false)
}
