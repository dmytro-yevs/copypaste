//! Storage of imported row references and their shared encrypted byte payload.
use super::{
    connection::write_tx,
    model::{NewItem, StoreError, StoredItem},
    Store,
};
use rusqlite::{params, Connection};

/// Returns the row portion of a hydrated encrypted reference, if present.
pub(super) fn row_ciphertext(bytes: &[u8]) -> &[u8] {
    crate::binary_reference::split(bytes).map_or(bytes, |(reference, _)| reference)
}

pub(super) fn link(tx: &Connection, id: &str, hash: &str, bytes: &[u8]) -> Result<(), StoreError> {
    if let Some((_, blob)) = crate::binary_reference::split(bytes) {
        if blob.is_empty() {
            return Err(StoreError::InvalidSchema);
        }
        tx.execute("INSERT INTO shared_binary_payloads (id, ciphertext) VALUES (?1, ?2) ON CONFLICT(id) DO NOTHING", params![hash, blob])?;
        tx.execute("INSERT INTO history_file_payloads (item_id, blob_id) VALUES (?1, ?2) ON CONFLICT(item_id) DO UPDATE SET blob_id = excluded.blob_id", params![id, hash])?;
    }
    Ok(())
}

impl Store {
    #[cfg(test)]
    pub(crate) fn imported_payload_count(&self) -> u64 {
        self.conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM shared_binary_payloads", [], |row| {
                row.get(0)
            })
            .unwrap()
    }
    pub(crate) fn insert_imported_file(&self, item: NewItem) -> Result<StoredItem, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute("INSERT INTO clipboard_items (id, content_ciphertext, nonce, content_type, content_hash, created_at, payload_metadata, content_bytes) VALUES (?1, ?2, X'', ?3, ?4, ?5, ?6, ?7)", params![item.id, row_ciphertext(&item.content_ciphertext), item.content_type, item.content_hash, item.created_at, item.payload_metadata, item.content_ciphertext.len() + item.payload_metadata.as_ref().map_or(0, String::len)])?;
        link(&tx, &item.id, &item.content_hash, &item.content_ciphertext)?;
        tx.commit()?;
        drop(conn);
        self.get(&item.id)?.ok_or(StoreError::NotFound)
    }
}
