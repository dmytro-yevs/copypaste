//! Chunk persistence shares the History transaction, encryption and retention.
use super::{connection::write_tx, NewItem, Store, StoreError, StoredItem};
use crate::{
    file_transfer::{chunk_id, FileTransfer, FileTransferError, CHUNK_BYTES},
    ItemKey,
};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::io::Read;
use zeroize::Zeroizing;

pub(crate) struct EncryptedFileChunk {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

impl Store {
    pub(crate) fn clear_file_stages(&self) -> Result<(), StoreError> {
        self.conn()?
            .execute("DELETE FROM file_transfer_staging", [])?;
        Ok(())
    }
    pub(crate) fn legacy_file_after(&self, after: &str) -> Result<Option<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let id:Option<String>=conn.query_row("SELECT id FROM clipboard_items WHERE deleted=0 AND content_type='file' AND id>?1 AND json_extract(payload_metadata,'$.file.transfer') IS NULL ORDER BY id LIMIT 1",[after],|row|row.get(0)).optional()?;
        drop(conn);
        id.map(|id| self.get(&id)).transpose().map(Option::flatten)
    }

    #[cfg(test)]
    pub(crate) fn file_transfer_blob_count(&self) -> u64 {
        self.conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(DISTINCT blob_id) FROM file_transfer_files",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    pub(crate) fn file_work_directory(&self) -> Option<&std::path::Path> {
        self.path.as_ref().and_then(|path| path.parent())
    }
    pub fn set_downloaded_file(&self, id: &str, path: &str) -> Result<(), StoreError> {
        self.conn()?.execute("INSERT INTO downloaded_files (item_id,destination) VALUES (?1,?2) ON CONFLICT(item_id,file_index) DO UPDATE SET destination=excluded.destination",params![id,path])?;
        Ok(())
    }
    pub fn downloaded_file(&self, id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT destination FROM downloaded_files WHERE item_id=?1 AND file_index=0",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub(crate) fn insert_file_transfer(
        &self,
        mut item: NewItem,
        reader: &mut impl Read,
        offer: &FileTransfer,
        key: &ItemKey,
        explicit: bool,
        replace: bool,
    ) -> Result<StoredItem, FileTransferError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn).map_err(StoreError::from)?;
        if !explicit && !replace {
            let existing: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE id=?1 AND deleted=0)",
                    [&item.id],
                    |r| r.get(0),
                )
                .map_err(StoreError::from)?;
            if existing {
                let previous: Option<String> = tx
                    .query_row(
                        "SELECT payload_metadata FROM clipboard_items WHERE id=?1",
                        [&item.id],
                        |r| r.get(0),
                    )
                    .map_err(StoreError::from)?;
                if let (Some(mut next), Some(previous)) = (
                    item.payload_metadata
                        .as_deref()
                        .and_then(|json| crate::PayloadMetadata::from_json(json, "file")),
                    previous
                        .as_deref()
                        .and_then(|json| crate::PayloadMetadata::from_json(json, "file")),
                ) {
                    next.merge_privacy(&previous);
                    item.payload_metadata = next.to_json("file");
                }
                tx.execute(
                    "UPDATE clipboard_items SET created_at=MAX(created_at + 1, ?2),app_bundle_id=?3,app_name=?4,payload_metadata=?5,content_bytes=?6 WHERE id=?1",
                    params![item.id, item.created_at,item.app_bundle_id,item.app_name,item.payload_metadata,offer.size_bytes+item.content_ciphertext.len() as u64+item.payload_metadata.as_ref().map_or(0,|m|m.len() as u64)],
                )
                .map_err(StoreError::from)?;
                tx.commit().map_err(StoreError::from)?;
                drop(conn);
                return self.get(&item.id)?.ok_or(FileTransferError::NotFound);
            }
            // Recapturing a previously deleted offer is a new live row.
            tx.execute(
                "DELETE FROM clipboard_items WHERE id=?1 AND deleted=1",
                [&item.id],
            )
            .map_err(StoreError::from)?;
        }
        if replace {
            tx.execute("UPDATE clipboard_items SET content_ciphertext=?2,nonce=X'',content_hash=?3,created_at=?4,payload_metadata=?5,content_bytes=?6 WHERE id=?1 AND deleted=0",params![item.id,item.content_ciphertext,item.content_hash,item.created_at,item.payload_metadata,offer.size_bytes+item.content_ciphertext.len() as u64+item.payload_metadata.as_ref().map_or(0,|m|m.len() as u64)]).map_err(StoreError::from)?;
        } else {
            tx.execute("INSERT INTO clipboard_items (id,content_ciphertext,nonce,content_type,content_hash,created_at,app_bundle_id,app_name,payload_metadata,content_bytes) VALUES (?1,?2,X'',?3,?4,?5,?6,?7,?8,?9)", params![item.id,item.content_ciphertext,item.content_type,item.content_hash,item.created_at,item.app_bundle_id,item.app_name,item.payload_metadata,offer.size_bytes + item.content_ciphertext.len() as u64 + item.payload_metadata.as_ref().map_or(0,|m|m.len() as u64)]).map_err(StoreError::from)?;
        }

        write_body(&tx, reader, offer, key)?;
        tx.execute(
            "INSERT INTO file_transfer_files VALUES (?1,?2)",
            params![item.id, offer.sha256],
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        drop(conn);
        self.get(&item.id)?.ok_or(FileTransferError::NotFound)
    }

    /// Keep an encrypted copy of member `index` beside its file reference.
    /// The synced version is unchanged, so peers see no new offer. False
    /// when the item changed or the member already has a copy.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attach_file_copy(
        &self,
        id: &str,
        index: usize,
        version_hash: &str,
        reader: &mut impl Read,
        body: &FileTransfer,
        key: &ItemKey,
    ) -> Result<bool, FileTransferError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn).map_err(StoreError::from)?;
        let current: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clipboard_items i JOIN file_references r ON r.item_id=i.id AND r.file_index=?2 WHERE i.id=?1 AND i.deleted=0 AND i.content_hash=?3 AND NOT EXISTS(SELECT 1 FROM file_copies k WHERE k.item_id=i.id AND k.file_index=?2))",
                params![id, index as i64, version_hash],
                |r| r.get(0),
            )
            .map_err(StoreError::from)?;
        if !current {
            return Ok(false);
        }
        write_body(&tx, reader, body, key)?;
        tx.execute(
            "INSERT INTO file_copies (item_id,file_index,blob_id) VALUES (?1,?2,?3)",
            params![id, index as i64, body.sha256],
        )
        .map_err(StoreError::from)?;
        tx.execute(
            "UPDATE clipboard_items SET content_bytes=content_bytes+?2 WHERE id=?1",
            params![id, body.size_bytes],
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        Ok(true)
    }

    /// The content-addressed body of a live item's kept copy of one member.
    pub fn file_copy_blob(&self, id: &str, index: usize) -> Result<Option<String>, StoreError> {
        Ok(self.conn()?.query_row("SELECT k.blob_id FROM file_copies k JOIN clipboard_items i ON i.id=k.item_id WHERE k.item_id=?1 AND k.file_index=?2 AND i.deleted=0",params![id, index as i64],|r|r.get(0)).optional()?)
    }

    /// Kept copies of a live item: member index and body, in set order.
    pub fn file_copies(&self, id: &str) -> Result<Vec<(usize, String)>, StoreError> {
        let conn = self.conn()?;
        let mut query = conn.prepare("SELECT k.file_index,k.blob_id FROM file_copies k JOIN clipboard_items i ON i.id=k.item_id WHERE k.item_id=?1 AND i.deleted=0 ORDER BY k.file_index")?;
        let rows = query.query_map([id], |r| Ok((r.get::<_, i64>(0)? as usize, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub(crate) fn file_copy_chunk(
        &self,
        id: &str,
        index: usize,
        blob: &str,
        chunk: u64,
    ) -> Result<Option<EncryptedFileChunk>, StoreError> {
        Ok(self.conn()?.query_row("SELECT c.nonce,c.ciphertext FROM file_transfer_chunks c JOIN file_copies k ON k.blob_id=c.blob_id WHERE k.item_id=?1 AND k.file_index=?2 AND c.blob_id=?3 AND c.chunk_index=?4",params![id,index as i64,blob,chunk],|r|Ok(EncryptedFileChunk {nonce:r.get(0)?,ciphertext:r.get(1)?})).optional()?)
    }

    /// Live file items made on this device that keep encrypted copies,
    /// newest first.
    pub fn local_file_copies_since(
        &self,
        local_device: &str,
        since_ms: i64,
        limit: usize,
    ) -> Result<Vec<StoredItem>, StoreError> {
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut statement = conn.prepare("SELECT i.id FROM clipboard_items i WHERE i.deleted=0 AND i.content_type='file' AND (i.origin_device_id='' OR i.origin_device_id=?1) AND i.created_at>=?2 AND EXISTS(SELECT 1 FROM file_copies k WHERE k.item_id=i.id) ORDER BY i.created_at DESC LIMIT ?3")?;
            let rows =
                statement.query_map(params![local_device, since_ms, limit as i64], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        Ok(ids
            .iter()
            .filter_map(|id| self.get(id).ok().flatten())
            .collect())
    }

    pub fn has_file_transfer(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM file_transfer_files f JOIN clipboard_items i ON i.id=f.item_id WHERE f.item_id=?1 AND i.deleted=0)",[id],|r|r.get(0))?)
    }
    pub(crate) fn file_transfer_chunk(
        &self,
        id: &str,
        hash: &str,
        index: u64,
    ) -> Result<Option<EncryptedFileChunk>, StoreError> {
        Ok(self.conn()?.query_row("SELECT c.nonce,c.ciphertext FROM file_transfer_chunks c JOIN file_transfer_files f ON f.blob_id=c.blob_id WHERE f.item_id=?1 AND c.blob_id=?2 AND c.chunk_index=?3",params![id,hash,index],|r|Ok(EncryptedFileChunk {nonce:r.get(0)?,ciphertext:r.get(1)?})).optional()?)
    }
}

fn write_body(
    tx: &rusqlite::Connection,
    reader: &mut impl Read,
    offer: &FileTransfer,
    key: &ItemKey,
) -> Result<(), FileTransferError> {
    let chunks: u64 = tx
        .query_row(
            "SELECT COUNT(*) FROM file_transfer_chunks WHERE blob_id=?1",
            [&offer.sha256],
            |r| r.get(0),
        )
        .map_err(StoreError::from)?;
    let reusable = chunks == offer.size_bytes.div_ceil(CHUNK_BYTES as u64);
    let mut buffer = Zeroizing::new(vec![0; CHUNK_BYTES]);
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut index = 0u64;
    loop {
        let mut n = 0;
        while n < buffer.len() {
            let count = reader
                .read(&mut buffer[n..])
                .map_err(|_| FileTransferError::Read)?;
            if count == 0 {
                break;
            }
            n += count;
        }
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > offer.size_bytes {
            return Err(FileTransferError::Read);
        }
        hash.update(&buffer[..n]);
        if reusable {
            let (nonce,encrypted):(Vec<u8>,Vec<u8>)=tx.query_row("SELECT nonce,ciphertext FROM file_transfer_chunks WHERE blob_id=?1 AND chunk_index=?2",params![offer.sha256,index],|r|Ok((r.get(0)?,r.get(1)?))).map_err(StoreError::from)?;
            let stored = crate::decrypt(&encrypted, &nonce, key, &chunk_id(&offer.sha256, index))?;
            if *stored != buffer[..n] {
                return Err(FileTransferError::Integrity);
            }
        } else {
            let (nonce, encrypted) =
                crate::encrypt(&buffer[..n], key, &chunk_id(&offer.sha256, index))?;
            tx.execute(
                "INSERT INTO file_transfer_chunks VALUES (?1,?2,?3,?4)",
                params![offer.sha256, index, encrypted, nonce.as_slice()],
            )
            .map_err(StoreError::from)?;
        }
        index += 1;
    }
    if total != offer.size_bytes || hex::encode(hash.finalize()) != offer.sha256 {
        return Err(FileTransferError::Read);
    }
    Ok(())
}
