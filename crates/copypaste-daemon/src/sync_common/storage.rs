//! Atomic LWW replace-by-`item_id` for cloud/relay-downloaded rows.
//!
//! Split out of the former flat `sync_common.rs` (ADR-017, CopyPaste-vp63.7)
//! — moved verbatim, no behavior change.

use copypaste_core::{ClipboardItem, Database};

/// Atomically replace a cloud/relay-downloaded clipboard row by its cross-device
/// `item_id`, preserving the row's primary key (`item.id`) so FTS / copy_item /
/// pins keep pointing at the same row.
///
/// Runs DELETE-by-item_id + INSERT inside one `unchecked_transaction` so a
/// failed insert rolls back the delete and the prior row survives.
pub(crate) fn replace_cloud_item_by_item_id(
    db: &Database,
    item: &ClipboardItem,
) -> anyhow::Result<()> {
    use rusqlite::{params, OptionalExtension};
    let tx = db.conn().unchecked_transaction()?;
    // e5oe: collect the row id(s) being replaced so we can delete the
    // matching clipboard_fts rows in the same transaction.  Without this, the
    // old plaintext content_text accumulates as an orphaned FTS row every time
    // a cloud/relay LWW overwrite lands.
    let old_id: Option<String> = tx
        .query_row(
            "SELECT id FROM clipboard_items WHERE item_id = ?1",
            params![item.item_id],
            |r| r.get(0),
        )
        .optional()?;
    tx.execute(
        "DELETE FROM clipboard_items WHERE item_id = ?1",
        params![item.item_id],
    )?;
    // Delete the corresponding FTS row (if any) in the same transaction.
    if let Some(ref old_id) = old_id {
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![old_id])?;
    }
    // CopyPaste-jvzm.1: clear any stale resumable-upload state for this item_id.
    // A prior upload session (tus_url / bytes_uploaded) describes the OLD content;
    // an LWW cloud/relay overwrite replaces that content, so resuming the old
    // session would push wrong/partial bytes — and if the row were never cleaned
    // it would be permanently stranded (never GC'd, possible infinite retry).
    // `pending_uploads.item_id` is the PRIMARY KEY, so this is a point delete,
    // mirroring the delete_expired / delete_item / prune_to_cap cleanup paths.
    tx.execute(
        "DELETE FROM pending_uploads WHERE item_id = ?1",
        params![item.item_id],
    )?;
    tx.execute(
        // CopyPaste-jvzm.2: list ALL persisted columns explicitly, including
        // `thumb` and `deleted`. Omitting them let SQLite apply column defaults
        // (thumb=NULL, deleted=0) on every LWW replace, which (a) dropped an
        // item's thumbnail and (b) — worse — silently un-deleted a tombstone
        // arriving via this path (deleted=0), so a cloud/relay-delivered deletion
        // would not stick. Setting them from the incoming item keeps the replace
        // faithful and guards against silent schema-drift when new columns are
        // added.
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            // Use the item's own key_version rather than the current constant
            // so cloud-synced items retain the key generation they were
            // encrypted with. ITEM_KEY_VERSION_CURRENT would silently stamp
            // v2 on v1-keyed chunks, poisoning future migration dispatches.
            item.key_version as i64,
            item.pinned as i64,
            item.pin_order,
            item.thumb,
            item.deleted as i64,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LWW fix: replace_cloud_item_by_item_id must store the item's own
    /// key_version, not the hardcoded ITEM_KEY_VERSION_CURRENT constant.
    /// A v1-keyed chunk item replaced via cloud LWW must survive as v1 so
    /// future migration dispatch can identify and re-encrypt it correctly.
    #[test]
    fn replace_cloud_item_preserves_key_version() {
        use copypaste_core::{get_item_by_item_id, insert_item, Database};

        let db = Database::open_in_memory().expect("in-memory DB");

        // Seed a v2 item that the remote will overwrite via LWW.
        let seed = ClipboardItem {
            id: "local-row-id".to_string().into(),
            item_id: "shared-item-id".to_string().into(),
            content_type: "text".to_string(),
            content: Some(b"old ciphertext".to_vec()),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "local-device".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        insert_item(&db, &seed).expect("insert seed");

        // Build a replacement that is v1-keyed (chunk from an older peer).
        let replacement = ClipboardItem {
            id: "local-row-id".to_string().into(),
            item_id: "shared-item-id".to_string().into(),
            content_type: "file".to_string(),
            content: None,
            content_nonce: None,
            blob_ref: Some("blob-abc".to_string()),
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 2,
            wall_time: 1_700_000_001_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "remote-device".to_string(),
            key_version: 1, // <-- must survive the LWW replace
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };

        replace_cloud_item_by_item_id(&db, &replacement).expect("replace");

        let stored = get_item_by_item_id(&db, "shared-item-id")
            .expect("query ok")
            .expect("row exists");

        assert_eq!(
            stored.key_version, 1,
            "replace_cloud_item_by_item_id must persist item.key_version, not ITEM_KEY_VERSION_CURRENT"
        );
    }

    /// e5oe: replace_cloud_item_by_item_id must NOT leave an orphaned FTS row
    /// after the replace.  Before the fix the old clipboard_fts row was never
    /// deleted, allowing stale plaintext to remain searchable.
    #[test]
    fn replace_cloud_item_removes_old_fts_row() {
        use copypaste_core::{insert_item_with_fts, Database};

        let db = Database::open_in_memory().expect("in-memory DB");

        let old_plaintext = "super secret old clipboard content";
        let seed = ClipboardItem {
            id: "fts-row-id".to_string().into(),
            item_id: "fts-item-id".to_string().into(),
            content_type: "text".to_string(),
            content: Some(b"old ciphertext".to_vec()),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-a".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        insert_item_with_fts(&db, &seed, old_plaintext).expect("insert with FTS");

        // Verify the FTS row exists before the replace.
        let fts_before: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                rusqlite::params!["fts-row-id"],
                |r| r.get(0),
            )
            .expect("count before");
        assert_eq!(fts_before, 1, "FTS row must exist before replace");

        // Replace with an item that has the same item_id but a different row id.
        let replacement = ClipboardItem {
            id: "fts-row-id-v2".to_string().into(),
            item_id: "fts-item-id".to_string().into(),
            content_type: "text".to_string(),
            content: Some(b"new ciphertext".to_vec()),
            content_nonce: Some(vec![1u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: true,
            lamport_ts: 2,
            wall_time: 1_700_000_001_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-b".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        replace_cloud_item_by_item_id(&db, &replacement).expect("replace");

        // The old FTS row must be gone (no orphan).
        let old_fts_after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                rusqlite::params!["fts-row-id"],
                |r| r.get(0),
            )
            .expect("count old id after");
        assert_eq!(
            old_fts_after, 0,
            "old FTS row must be deleted by replace_cloud_item_by_item_id (e5oe)"
        );
    }

    /// CopyPaste-jvzm.1: replacing a cloud item by item_id must also clear any
    /// stale `pending_uploads` row for that item_id, so a prior resumable-upload
    /// session cannot resume against the new content or be stranded forever.
    #[test]
    fn replace_cloud_item_clears_pending_uploads() {
        use copypaste_core::{insert_item, Database};

        let db = Database::open_in_memory().expect("in-memory DB");
        let item_id = "pu-item-id";

        let seed = ClipboardItem {
            id: "pu-row-id".to_string().into(),
            item_id: item_id.to_string().into(),
            content_type: "file".to_string(),
            content: Some(b"old ciphertext".to_vec()),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-a".to_string(),
            key_version: 2,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        };
        insert_item(&db, &seed).expect("insert seed");

        // Simulate an in-progress resumable upload for this item.
        db.conn()
            .execute(
                "INSERT INTO pending_uploads
                 (item_id, tus_url, bytes_uploaded, total_bytes, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    item_id,
                    "https://relay.example.com/tus/abc",
                    1024_i64,
                    4096_i64,
                    1_700_000_000_i64,
                    1_700_900_000_i64,
                ],
            )
            .expect("seed pending_uploads");
        let before: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM pending_uploads WHERE item_id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            )
            .expect("count before");
        assert_eq!(before, 1, "pending_uploads row must exist before replace");

        // LWW overwrite: same item_id, new content/row id.
        let replacement = ClipboardItem {
            id: "pu-row-id-v2".to_string().into(),
            is_synced: true,
            lamport_ts: 2,
            wall_time: 1_700_000_001_000,
            content: Some(b"new ciphertext".to_vec()),
            content_nonce: Some(vec![1u8; 24]),
            origin_device_id: "device-b".to_string(),
            ..seed.clone()
        };
        replace_cloud_item_by_item_id(&db, &replacement).expect("replace");

        let after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM pending_uploads WHERE item_id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            )
            .expect("count after");
        assert_eq!(
            after, 0,
            "stale pending_uploads row must be cleared by replace_cloud_item_by_item_id"
        );
    }

    /// CopyPaste-jvzm.2: the LWW replace INSERT must persist `deleted` and
    /// `thumb` from the incoming item, not silently default them. Regression:
    /// a cloud/relay-delivered tombstone (deleted=true) must STAY deleted after
    /// the replace, and a thumbnail must survive.
    #[test]
    fn replace_cloud_item_preserves_deleted_and_thumb() {
        use copypaste_core::{insert_item, Database};

        let db = Database::open_in_memory().expect("in-memory DB");
        let item_id = "jvzm2-iid";

        // Seed a live (not deleted) item with a thumbnail.
        let mut seed = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        seed.id = "jvzm2-row-v1".to_string().into();
        seed.item_id = item_id.to_string().into();
        seed.thumb = Some(vec![0xAB; 8]);
        insert_item(&db, &seed).expect("insert seed");

        // LWW replace with a TOMBSTONE (deleted=true) carrying a new thumb.
        let replacement = ClipboardItem {
            id: "jvzm2-row-v2".to_string().into(),
            deleted: true,
            thumb: Some(vec![0xCD; 4]),
            lamport_ts: 2,
            ..seed.clone()
        };
        replace_cloud_item_by_item_id(&db, &replacement).expect("replace");

        let (deleted, thumb_len): (i64, Option<i64>) = db
            .conn()
            .query_row(
                "SELECT deleted, LENGTH(thumb) FROM clipboard_items WHERE item_id = ?1",
                rusqlite::params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query replaced row");
        assert_eq!(deleted, 1, "tombstone must stay deleted after replace");
        assert_eq!(
            thumb_len,
            Some(4),
            "incoming thumbnail must be persisted by the replace"
        );
    }
}
