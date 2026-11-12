//! The receiving device's decision about one incoming clip version, made in
//! the transaction that applies it.

use rusqlite::{Connection, OptionalExtension};

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
    let expired = tx
        .query_row(
            "SELECT 1 FROM expired_ids WHERE clip_id = ?1",
            [incoming.id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if expired
        || (!incoming.deleted && super::expiry::arrived_expired(tx, incoming, crate::now_ms())?)
    {
        return Ok(ReceiptOutcome::Discard);
    }
    Ok(ReceiptOutcome::Store)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::store;
    use super::super::IncomingItem;

    #[test]
    fn a_rearrival_of_a_locally_expired_clip_is_reported_stored_and_not_kept() {
        let store = store();
        store
            .conn()
            .unwrap()
            .execute("INSERT INTO expired_ids VALUES ('gone', 1)", [])
            .unwrap();
        let stored = store
            .upsert(&IncomingItem {
                id: "gone",
                content_ciphertext: Some(&[1, 2, 3]),
                nonce: Some(&[4]),
                content_type: "text",
                content_hash: "hash",
                created_at: 10,
                deleted: false,
                origin_device_id: "peer",
                app_bundle_id: None,
                app_name: None,
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
                search_text: Some("expired secret"),
                payload_metadata: None,
            })
            .unwrap();
        assert!(stored);
        assert!(store.version("gone").unwrap().is_none());
        assert!(store.search("expired secret", 10).unwrap().is_empty());
    }
}
