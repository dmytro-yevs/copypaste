//! Explicit secrecy changes and their search-index updates share one transaction.

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::{item_columns, row_to_item, ItemColumns};
use super::search::{delete_fts_row_in_tx, insert_fts_in_tx};
use super::{Store, StoreError};
use crate::{Keyring, PayloadMetadata};

impl Store {
    /// Changes confidentiality without replacing content, identity, or pin state.
    pub fn set_secret(&self, keys: &Keyring, id: &str, secret: bool) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let item = {
            let mut stmt = tx.prepare_cached(concat!(
                "SELECT ",
                item_columns!(),
                " FROM clipboard_items WHERE id = ?1 AND deleted = 0"
            ))?;
            let columns = ItemColumns::resolve(&stmt)?;
            stmt.query_row([id], |row| row_to_item(row, &columns))
                .optional()?
        };
        let Some(item) = item else {
            return Ok(false);
        };
        let mut metadata = item
            .payload_metadata
            .as_deref()
            .and_then(|json| PayloadMetadata::from_json(json, &item.content_type))
            .unwrap_or_default();
        metadata.privacy = item.clipboard_privacy();
        let stamp = crate::now_ms().max(item.created_at.saturating_add(1)).max(
            metadata
                .secret_override
                .map_or(0, |choice| choice.updated_at.saturating_add(1)),
        );
        metadata.set_secret(secret, stamp);
        let json = metadata
            .to_json(&item.content_type)
            .ok_or(StoreError::InvalidPayload)?;
        let plaintext = if !secret && copypaste_ipc::content_type::is_text(&item.content_type) {
            Some(
                crate::decrypt(&item.content_ciphertext, &item.nonce, &keys.item_key(), id)
                    .map_err(|_| StoreError::InvalidPayload)?,
            )
        } else {
            None
        };
        delete_fts_row_in_tx(&tx, id)?;
        tx.execute(
            "DELETE FROM module_search_documents WHERE item_id = ?1",
            [id],
        )?;
        tx.execute("DELETE FROM module_search_vectors WHERE item_id = ?1", [id])?;
        let fts_rowid = match plaintext.as_deref() {
            Some(bytes) => insert_fts_in_tx(
                &tx,
                id,
                std::str::from_utf8(bytes).map_err(|_| StoreError::InvalidPayload)?,
                &item.content_type,
            )?,
            None => None,
        };
        tx.execute(
            "UPDATE clipboard_items SET payload_metadata = ?2, created_at = ?3, fts_rowid = ?4, \
             content_bytes = LENGTH(content_ciphertext) + LENGTH(?2) WHERE id = ?1",
            params![id, json, stamp, fts_rowid],
        )?;
        tx.commit()?;
        Ok(true)
    }
}
