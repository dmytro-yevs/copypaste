//! Disposable, authenticated payload previews for paged History reads.
use std::io::{Read, Write};

use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::model::{item_columns_ci, row_to_item, ItemColumns, StoreError, StoredItem};
use crate::crypto::{open_preview, seal_preview, PreviewKey};
use crate::history_item::StaticHistoryPreview;
use crate::{ClipboardPayload, ItemKey};

// Corrupt or oversized cache records must not allocate unbounded memory.
const MAX_CACHE_BYTES: i64 = 64 * 1024;
// Count and per-entry limits bound cached ciphertext to at most 64 MiB.
const MAX_CACHE_ENTRIES: i64 = 1024;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

struct BoundedPreviewWriter {
    bytes: Zeroizing<Vec<u8>>,
}

impl Write for BoundedPreviewWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_CACHE_BYTES as usize - 16 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "preview exceeds cache capacity",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) struct Pending {
    id: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}
pub(super) struct Resolved {
    pub preview: Option<StaticHistoryPreview>,
    pub pending: Option<Pending>,
}

fn hash_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn hash_blob(
    conn: &Connection,
    table: &str,
    column: &str,
    rowid: i64,
    hash: &mut Sha256,
) -> rusqlite::Result<()> {
    let mut blob = conn.blob_open(DatabaseName::Main, table, column, rowid, true)?;
    let mut buffer = [0u8; HASH_BUFFER_BYTES];
    loop {
        let count = blob
            .read(&mut buffer)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(())
}

fn fingerprint(conn: &Connection, row: &Row<'_>, item: &StoredItem) -> rusqlite::Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"copypaste/history-preview-source/v1");
    hash_field(&mut hash, item.id.as_bytes());
    hash_field(&mut hash, &item.nonce);
    hash_field(&mut hash, item.content_type.as_bytes());
    match &item.payload_metadata {
        Some(metadata) => {
            hash.update([1]);
            hash_field(&mut hash, metadata.as_bytes());
        }
        None => hash.update([0]),
    }
    let length: i64 = row.get("logical_cipher_bytes")?;
    hash.update(length.to_be_bytes());
    let source: i64 = row.get("source_rowid")?;
    let source_len: i64 = row.get("source_cipher_bytes")?;
    if source_len > 0 {
        hash_blob(
            conn,
            "clipboard_items",
            "content_ciphertext",
            source,
            &mut hash,
        )?;
    }
    if row.get::<_, bool>("source_present")? {
        if let Some(shared) = row.get::<_, Option<i64>>("shared_rowid")? {
            hash_blob(
                conn,
                "shared_binary_payloads",
                "ciphertext",
                shared,
                &mut hash,
            )?;
        }
    }
    Ok(hash.finalize().into())
}

pub(super) fn resolve(
    conn: &Connection,
    row: &Row<'_>,
    item: &StoredItem,
    item_key: &ItemKey,
    preview_key: &PreviewKey,
) -> Result<Resolved, StoreError> {
    // A cache/fingerprint failure is merely a miss. Only opening the original
    // can classify the row as unreadable.
    let fingerprint = fingerprint(conn, row, item).ok();
    if let Some(fingerprint) = &fingerprint {
        let cached: Option<(Vec<u8>, Vec<u8>)> = conn.query_row(
            "SELECT nonce, ciphertext FROM history_preview_cache WHERE item_id = ?1 AND length(nonce) = 24 AND length(ciphertext) <= ?2",
            params![item.id, MAX_CACHE_BYTES], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional().ok().flatten();
        if let Some((nonce, ciphertext)) = cached {
            if let Ok(plaintext) =
                open_preview(&ciphertext, &nonce, preview_key, &item.id, fingerprint)
            {
                if let Ok(preview) = serde_json::from_slice(&plaintext) {
                    return Ok(Resolved {
                        preview: Some(preview),
                        pending: None,
                    });
                }
            }
        }
    }
    let sql = format!(
        "SELECT {} FROM clipboard_items ci WHERE ci.id = ?1",
        item_columns_ci!()
    );
    let mut stmt = conn.prepare(&sql)?;
    let columns = ItemColumns::resolve(&stmt)?;
    let full = stmt.query_row([&item.id], |row| row_to_item(row, &columns))?;
    let payload = match ClipboardPayload::open(&full, item_key) {
        Ok(payload) => payload,
        Err(_) => {
            return Ok(Resolved {
                preview: None,
                pending: None,
            })
        }
    };
    let preview = StaticHistoryPreview::from_open_payload(&full, &payload);
    let pending = fingerprint.and_then(|fingerprint| {
        let mut serialized = BoundedPreviewWriter {
            bytes: Zeroizing::new(Vec::new()),
        };
        serde_json::to_writer(&mut serialized, &preview).ok()?;
        let (nonce, ciphertext) =
            seal_preview(&serialized.bytes, preview_key, &item.id, &fingerprint).ok()?;
        Some(Pending {
            id: item.id.clone(),
            nonce,
            ciphertext,
        })
    });
    Ok(Resolved {
        preview: Some(preview),
        pending,
    })
}

pub(super) fn store_pending(conn: &mut Connection, generation: &str, pending: Vec<Pending>) {
    if pending.is_empty() {
        return;
    }
    let timeout: i64 = conn
        .pragma_query_value(None, "busy_timeout", |r| r.get(0))
        .unwrap_or(5000);
    if conn.busy_timeout(std::time::Duration::ZERO).is_err() {
        return;
    }
    let result = (|| -> rusqlite::Result<()> {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for entry in pending {
            tx.execute("INSERT INTO history_preview_cache(item_id, nonce, ciphertext) SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM clipboard_items WHERE id=?1 AND deleted=0) AND EXISTS (SELECT 1 FROM resource_revisions WHERE singleton=0 AND generation=?4) ON CONFLICT(item_id) DO UPDATE SET nonce=excluded.nonce, ciphertext=excluded.ciphertext", params![entry.id, entry.nonce, entry.ciphertext, generation])?;
        }
        tx.execute(
            "DELETE FROM history_preview_cache WHERE rowid IN (SELECT rowid FROM history_preview_cache ORDER BY rowid DESC LIMIT -1 OFFSET ?1)",
            [MAX_CACHE_ENTRIES],
        )?;
        tx.commit()
    })();
    let _ = conn.busy_timeout(std::time::Duration::from_millis(timeout.max(0) as u64));
    if let Err(error) = result {
        tracing::debug!(%error, "skipping History preview cache writes");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, store, T0};
    use crate::{HistoryQuery, Keyring};

    fn inserted(store: &crate::Store, keys: &Keyring, text: &str, time: i64) -> String {
        let mut row = item(text, time);
        let (nonce, ciphertext) =
            crate::encrypt(text.as_bytes(), &keys.item_key(), &row.id).unwrap();
        row.nonce = nonce;
        row.content_ciphertext = ciphertext;
        store.insert(row).unwrap().id
    }

    fn query(store: &crate::Store, keys: &Keyring) -> crate::HistoryPreviewPage {
        store
            .query_history_previews_with_semantic(
                &HistoryQuery::default(),
                None,
                20,
                usize::MAX,
                None,
                None,
                &keys.item_key(),
                &keys.preview_key(),
            )
            .unwrap()
    }

    #[test]
    fn cache_hit_matches_fallback_and_invalid_source_is_not_hidden() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "private test text", T0);
        let first = query(&store, &keys);
        let nonce: Vec<u8> = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT nonce FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        let second = query(&store, &keys);
        let after: Vec<u8> = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT nonce FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(nonce, after, "a valid cache entry must be reused");
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: None,
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let before = first.items[0].preview.clone().unwrap().into_item(
            &first.items[0].row,
            &origin,
            "local",
        );
        let after = second.items[0].preview.clone().unwrap().into_item(
            &second.items[0].row,
            &origin,
            "local",
        );
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(after).unwrap()
        );
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET content_ciphertext = X'1234' WHERE id=?1",
                [&id],
            )
            .unwrap();
        assert!(query(&store, &keys).items[0].preview.is_none());
    }

    #[test]
    fn damaged_cache_and_changed_metadata_fall_back_without_skipping() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "hello", T0);
        assert!(query(&store, &keys).items[0].preview.is_some());
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE history_preview_cache SET ciphertext=X'00' WHERE item_id=?1",
                [&id],
            )
            .unwrap();
        assert!(query(&store, &keys).items[0].preview.is_some());
        let conn = store.conn().unwrap();
        conn.execute(
            "UPDATE clipboard_items SET payload_metadata='{}' WHERE id=?1",
            [&id],
        )
        .unwrap();
        conn.execute_batch("CREATE TRIGGER fail_preview BEFORE INSERT ON history_preview_cache BEGIN SELECT RAISE(ABORT, 'no cache writes'); END;").unwrap();
        drop(conn);
        assert!(query(&store, &keys).items[0].preview.is_some());
    }

    #[test]
    fn mutable_row_metadata_and_secret_status_are_not_stale() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "sensitive@example.org", T0);
        assert!(query(&store, &keys).items[0].preview.is_some());
        let conn = store.conn().unwrap();
        conn.execute("UPDATE clipboard_items SET pinned=1, app_name='Changed', payload_metadata=?1 WHERE id=?2", params![r#"{"privacy":{"secret":true}}"#, id]).unwrap();
        drop(conn);
        let result = query(&store, &keys);
        let entry = &result.items[0];
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: Some("New device".into()),
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let rendered = entry
            .preview
            .clone()
            .unwrap()
            .into_item(&entry.row, &origin, "local");
        assert!(rendered.pinned);
        assert_eq!(rendered.source_app_name.as_deref(), Some("Changed"));
        assert_eq!(rendered.origin_device_name.as_deref(), Some("New device"));
        assert!(rendered.privacy.secret);
        assert!(rendered.content.is_empty());
        assert!(rendered.semantic_kind.is_none());
    }

    #[test]
    fn metadata_only_changes_are_visible_on_a_cache_hit() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "fresh metadata", T0);
        query(&store, &keys);
        let conn = store.conn().unwrap();
        let original_nonce: Vec<u8> = conn
            .query_row(
                "SELECT nonce FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE clipboard_items SET pinned=1, app_name='New app' WHERE id=?1",
            [&id],
        )
        .unwrap();
        drop(conn);
        let page = query(&store, &keys);
        let current_nonce: Vec<u8> = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT nonce FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(original_nonce, current_nonce);
        let entry = &page.items[0];
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: Some("New device".into()),
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let rendered = entry
            .preview
            .clone()
            .unwrap()
            .into_item(&entry.row, &origin, "local");
        assert!(rendered.pinned);
        assert_eq!(rendered.source_app_name.as_deref(), Some("New app"));
        assert_eq!(rendered.origin_device_name.as_deref(), Some("New device"));
    }

    #[test]
    fn file_source_availability_is_recomputed_for_each_cache_hit() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.txt");
        std::fs::write(&path, b"file data").unwrap();
        let mut row = item("file", T0);
        row.content_type = "file".into();
        row.search_text = None;
        row.content_ciphertext =
            crate::binary::seal(b"file data", &keys.item_key(), &row.id).unwrap();
        let file = crate::FileMetadata {
            filename: "source.txt".into(),
            mime_type: "text/plain".into(),
            source_reference: Some(path.to_string_lossy().into_owned()),
            transfer: None,
        };
        row.payload_metadata = crate::PayloadMetadata::new(Some(file), None)
            .unwrap()
            .to_json("file");
        store.insert(row).unwrap();
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: None,
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let first = query(&store, &keys);
        let item = first.items[0].preview.clone().unwrap().into_item(
            &first.items[0].row,
            &origin,
            "local",
        );
        assert!(item.file_details.unwrap().source_available);
        std::fs::remove_file(path).unwrap();
        let second = query(&store, &keys);
        let item = second.items[0].preview.clone().unwrap().into_item(
            &second.items[0].row,
            &origin,
            "local",
        );
        assert!(!item.file_details.unwrap().source_available);
    }

    #[test]
    fn rebound_shared_binary_payload_cannot_reuse_a_cached_preview() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let mut row = item("binary", T0);
        row.id = "file-import-one".into();
        row.content_type = "image/png".into();
        row.search_text = None;
        let bytes = b"first binary payload";
        let digest = crate::binary::content_digest(bytes);
        row.content_ciphertext =
            crate::binary_reference::seal(bytes, &digest, &keys.item_key(), &row.id).unwrap();
        let (reference, blob) = crate::binary_reference::split(&row.content_ciphertext).unwrap();
        let reference = reference.to_vec();
        let blob = blob.to_vec();
        let id = store.insert(row).unwrap().id;
        let conn = store.conn().unwrap();
        conn.execute(
            "UPDATE clipboard_items SET content_ciphertext=?1 WHERE id=?2",
            params![reference, id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO shared_binary_payloads VALUES ('binary-a', ?1)",
            [&blob],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO history_file_payloads VALUES (?1, 'binary-a')",
            [&id],
        )
        .unwrap();
        drop(conn);
        assert!(query(&store, &keys).items[0].preview.is_some());
        let conn = store.conn().unwrap();
        conn.execute(
            "INSERT INTO shared_binary_payloads VALUES ('binary-b', X'010203')",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE history_file_payloads SET blob_id='binary-b' WHERE item_id=?1",
            [&id],
        )
        .unwrap();
        drop(conn);
        assert!(query(&store, &keys).items[0].preview.is_none());
    }

    #[test]
    fn preview_pages_preserve_full_query_cursors_and_budget() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        for (index, text) in ["alpha result", "beta result", "gamma result"]
            .iter()
            .enumerate()
        {
            inserted(&store, &keys, text, T0 + index as i64);
        }
        for sort in [
            copypaste_ipc::HistorySort::Newest,
            copypaste_ipc::HistorySort::Oldest,
            copypaste_ipc::HistorySort::Relevance,
        ] {
            let query = HistoryQuery {
                search: (sort == copypaste_ipc::HistorySort::Relevance).then(|| "result".into()),
                sort,
                ..Default::default()
            };
            let mut cursor = None;
            loop {
                let full = store
                    .query_history_with_semantic(&query, cursor.as_ref(), 2, 40, None, None)
                    .unwrap();
                let preview = store
                    .query_history_previews_with_semantic(
                        &query,
                        cursor.as_ref(),
                        2,
                        40,
                        None,
                        None,
                        &keys.item_key(),
                        &keys.preview_key(),
                    )
                    .unwrap();
                assert_eq!(
                    full.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
                    preview
                        .items
                        .iter()
                        .map(|item| &item.row.id)
                        .collect::<Vec<_>>()
                );
                assert_eq!(full.next, preview.next);
                assert!(preview.items.iter().all(|item| item.preview.is_some()));
                cursor = full.next;
                if cursor.is_none() {
                    break;
                }
            }
        }
    }

    #[test]
    fn semantic_matches_use_the_same_ranking_and_cursor() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let matched = inserted(&store, &keys, "lexical needle", T0);
        let semantic = inserted(&store, &keys, "meaning only", T0 + 1);
        let query = HistoryQuery {
            search: Some("needle".into()),
            sort: copypaste_ipc::HistorySort::Relevance,
            ..Default::default()
        };
        let matches = crate::SemanticMatches {
            scope: "test-model".into(),
            scores: vec![(matched, 0.7), (semantic, 0.9)].into(),
        };
        let mut cursor = None;
        loop {
            let full = store
                .query_history_with_semantic(
                    &query,
                    cursor.as_ref(),
                    1,
                    usize::MAX,
                    None,
                    Some(&matches),
                )
                .unwrap();
            let previews = store
                .query_history_previews_with_semantic(
                    &query,
                    cursor.as_ref(),
                    1,
                    usize::MAX,
                    None,
                    Some(&matches),
                    &keys.item_key(),
                    &keys.preview_key(),
                )
                .unwrap();
            assert_eq!(
                full.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
                previews
                    .items
                    .iter()
                    .map(|item| &item.row.id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(full.next, previews.next);
            cursor = full.next;
            if cursor.is_none() {
                break;
            }
        }
    }

    #[test]
    fn legacy_list_preview_matches_original_cursor_and_budget() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        for (index, text) in ["alpha", "beta", "gamma"].iter().enumerate() {
            let id = inserted(&store, &keys, text, T0 + index as i64);
            if index == 0 {
                store.set_pinned(&id, true).unwrap();
            }
        }
        let mut cursor = None;
        loop {
            let full = store.list_from_bounded(cursor.as_ref(), 2, 30).unwrap();
            let preview = store
                .list_previews_bounded(
                    cursor.as_ref(),
                    2,
                    30,
                    &keys.item_key(),
                    &keys.preview_key(),
                )
                .unwrap();
            assert_eq!(
                full.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
                preview
                    .items
                    .iter()
                    .map(|item| &item.row.id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(full.next, preview.next);
            assert!(preview.items.iter().all(|item| item.preview.is_some()));
            cursor = full.next;
            if cursor.is_none() {
                break;
            }
        }
    }

    #[test]
    fn restore_clears_previews_and_rotates_generation() {
        let dir = tempfile::tempdir().unwrap();
        let keys = Keyring::from_secret(&[42; 32]);
        let store = crate::Store::open(&dir.path().join("live.db"), &keys.db_key()).unwrap();
        inserted(&store, &keys, "original", T0);
        assert!(query(&store, &keys).items[0].preview.is_some());
        let backup = dir.path().join("backup.db");
        store.backup_to(&backup).unwrap();
        inserted(&store, &keys, "later", T0 + 1);
        assert_eq!(query(&store, &keys).items.len(), 2);
        let generation = store.resource_revisions().unwrap().generation;
        store.restore_from(&backup, &keys.db_key()).unwrap();
        assert_ne!(store.resource_revisions().unwrap().generation, generation);
        let count: i64 = store
            .conn()
            .unwrap()
            .query_row("SELECT count(*) FROM history_preview_cache", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        let after = query(&store, &keys);
        assert_eq!(after.items.len(), 1);
        assert!(after.items[0].preview.is_some());
    }

    #[test]
    fn tombstone_removes_its_cached_preview() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "delete this", T0);
        query(&store, &keys);
        assert!(store.delete(&id).unwrap());
        let count: i64 = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn stale_concurrent_cache_write_cannot_hide_a_new_payload() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "before", T0);
        query(&store, &keys);
        let conn = store.conn().unwrap();
        let (nonce, ciphertext): (Vec<u8>, Vec<u8>) = conn
            .query_row(
                "SELECT nonce,ciphertext FROM history_preview_cache WHERE item_id=?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let (new_nonce, new_ciphertext) = crate::encrypt(b"after", &keys.item_key(), &id).unwrap();
        conn.execute(
            "UPDATE clipboard_items SET nonce=?1, content_ciphertext=?2 WHERE id=?3",
            params![new_nonce, new_ciphertext, id],
        )
        .unwrap();
        conn.execute(
            "UPDATE history_preview_cache SET nonce=?1,ciphertext=?2 WHERE item_id=?3",
            params![nonce, ciphertext, id],
        )
        .unwrap();
        drop(conn);
        let page = query(&store, &keys);
        let origin = crate::history_item::ItemOrigin {
            device_id: "local".into(),
            device_name: None,
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let item =
            page.items[0]
                .preview
                .clone()
                .unwrap()
                .into_item(&page.items[0].row, &origin, "local");
        assert_eq!(item.content, "after");
    }

    #[test]
    fn preview_serialization_cannot_exceed_the_cache_byte_limit() {
        let mut writer = BoundedPreviewWriter {
            bytes: Zeroizing::new(Vec::new()),
        };
        let oversized = "x".repeat(MAX_CACHE_BYTES as usize * 2);
        assert!(serde_json::to_writer(&mut writer, &oversized).is_err());
        assert!(writer.bytes.len() <= MAX_CACHE_BYTES as usize - 16);
    }

    #[test]
    fn cache_retains_a_bounded_number_of_entries() {
        let store = store();
        let mut conn = store.conn().unwrap();
        conn.execute_batch("WITH RECURSIVE n(i) AS (VALUES(0) UNION ALL SELECT i+1 FROM n WHERE i<1024) INSERT INTO clipboard_items(id, content_type, created_at) SELECT 'dummy-' || i, 'text', 1 FROM n; INSERT INTO history_preview_cache(item_id, nonce, ciphertext) SELECT id, zeroblob(24), zeroblob(16) FROM clipboard_items;").unwrap();
        let generation: String = conn
            .query_row(
                "SELECT generation FROM resource_revisions WHERE singleton=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        store_pending(
            &mut conn,
            &generation,
            vec![Pending {
                id: "dummy-1024".into(),
                nonce: vec![1; 24],
                ciphertext: vec![2; 16],
            }],
        );
        let count: i64 = conn
            .query_row("SELECT count(*) FROM history_preview_cache", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, MAX_CACHE_ENTRIES);
    }

    #[test]
    fn writes_from_an_old_restore_generation_are_discarded() {
        let store = store();
        let keys = Keyring::from_secret(&[42; 32]);
        let id = inserted(&store, &keys, "hello", T0);
        let mut conn = store.conn().unwrap();
        let previous: String = conn
            .query_row("SELECT generation FROM resource_revisions", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "UPDATE resource_revisions SET generation='new' WHERE singleton=0",
            [],
        )
        .unwrap();
        store_pending(
            &mut conn,
            &previous,
            vec![Pending {
                id,
                nonce: vec![0; 24],
                ciphertext: vec![0; 16],
            }],
        );
        let count: i64 = conn
            .query_row("SELECT count(*) FROM history_preview_cache", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
