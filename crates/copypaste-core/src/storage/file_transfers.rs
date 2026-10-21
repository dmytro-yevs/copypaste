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
    pub(crate) fn stage_file_transfer(
        &self,
        reader: &mut impl Read,
        offer: &FileTransfer,
        key: &ItemKey,
    ) -> Result<String, FileTransferError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn).map_err(StoreError::from)?;
        write_body(&tx, reader, offer, key)?;
        let lease = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO file_transfer_staging VALUES (?1,?2)",
            params![lease, offer.sha256],
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        Ok(lease)
    }
    pub(crate) fn release_file_stage(&self, lease: &str) -> Result<(), StoreError> {
        self.conn()?.execute(
            "DELETE FROM file_transfer_staging WHERE lease_id=?1",
            [lease],
        )?;
        Ok(())
    }
    pub(crate) fn clear_file_stages(&self) -> Result<(), StoreError> {
        self.conn()?
            .execute("DELETE FROM file_transfer_staging", [])?;
        Ok(())
    }
    pub(crate) fn publish_file_stage(
        &self,
        mut item: NewItem,
        lease: &str,
        offer: &FileTransfer,
    ) -> Result<(StoredItem, bool), FileTransferError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn).map_err(StoreError::from)?;
        let staged:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM file_transfer_staging WHERE lease_id=?1 AND blob_id=?2)",params![lease,offer.sha256],|r|r.get(0)).map_err(StoreError::from)?;
        if !staged {
            return Err(FileTransferError::Integrity);
        }
        let previous: Option<Option<String>> = tx
            .query_row(
                "SELECT payload_metadata FROM clipboard_items WHERE id=?1 AND deleted=0",
                [&item.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(StoreError::from)?;
        let duplicate = previous.is_some();
        if let Some(previous) = previous {
            if let (Some(mut next), Some(previous)) = (
                item.payload_metadata
                    .as_deref()
                    .and_then(|m| crate::PayloadMetadata::from_json(m, "file")),
                previous
                    .as_deref()
                    .and_then(|m| crate::PayloadMetadata::from_json(m, "file")),
            ) {
                next.merge_privacy(&previous);
                item.payload_metadata = next.to_json("file");
            }
            tx.execute("UPDATE clipboard_items SET created_at=MAX(created_at+1,?2),app_bundle_id=?3,app_name=?4,payload_metadata=?5 WHERE id=?1",params![item.id,item.created_at,item.app_bundle_id,item.app_name,item.payload_metadata]).map_err(StoreError::from)?;
        } else {
            tx.execute(
                "DELETE FROM clipboard_items WHERE id=?1 AND deleted=1",
                [&item.id],
            )
            .map_err(StoreError::from)?;
            tx.execute("INSERT INTO clipboard_items (id,content_ciphertext,nonce,content_type,content_hash,created_at,app_bundle_id,app_name,payload_metadata,content_bytes) VALUES (?1,?2,X'','file',?3,?4,?5,?6,?7,?8)",params![item.id,item.content_ciphertext,item.content_hash,item.created_at,item.app_bundle_id,item.app_name,item.payload_metadata,offer.size_bytes+item.content_ciphertext.len() as u64+item.payload_metadata.as_ref().map_or(0,|m|m.len() as u64)]).map_err(StoreError::from)?;
        }
        tx.execute("INSERT INTO file_transfer_files VALUES (?1,?2) ON CONFLICT(item_id) DO UPDATE SET blob_id=excluded.blob_id",params![item.id,offer.sha256]).map_err(StoreError::from)?;
        tx.execute(
            "DELETE FROM file_transfer_staging WHERE lease_id=?1",
            [lease],
        )
        .map_err(StoreError::from)?;
        tx.commit().map_err(StoreError::from)?;
        drop(conn);
        Ok((
            self.get(&item.id)?.ok_or(FileTransferError::NotFound)?,
            duplicate,
        ))
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
        self.conn()?.execute("INSERT INTO downloaded_files (item_id,destination) VALUES (?1,?2) ON CONFLICT(item_id) DO UPDATE SET destination=excluded.destination",params![id,path])?;
        Ok(())
    }
    pub fn downloaded_file(&self, id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT destination FROM downloaded_files WHERE item_id=?1",
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
