//! Storage of imported row references and their shared encrypted byte payload.
use super::model::StoreError;
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
