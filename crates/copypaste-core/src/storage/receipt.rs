//! The receiving device's decision about one incoming clip version, made in
//! the transaction that applies it.

use rusqlite::Connection;

use super::{IncomingItem, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiptOutcome {
    /// Write the version.
    Store,
    /// Persist nothing and report the version as stored, so the sender's
    /// cursor advances past it.
    Discard,
}

pub(crate) fn on_received(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<ReceiptOutcome, StoreError> {
    if !incoming.deleted && super::expiry::arrived_expired(tx, incoming, crate::now_ms())? {
        return Ok(ReceiptOutcome::Discard);
    }
    Ok(ReceiptOutcome::Store)
}
