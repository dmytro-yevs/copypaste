//! Item CRUD: everything that writes or reads a `clipboard_items` row without
//! going through FTS or a retention sweep. Two invariants live here: layer 1 of
//! the ADR-015 sensitive/FTS exclusion (in [`Store::insert_or_bump`]), and that
//! a delete is a *tombstone*, not a row removal.

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::{
    is_constraint_violation, item_columns, row_to_item, Ingest, NewItem, StoreError, StoredItem,
};
use super::retention::{bump_in_tx, find_in_bucket, newest_live_with_hash};
use super::search::upsert_fts_in_tx;
use super::store::Store;

fn promote_sensitive_in_tx(
    tx: &rusqlite::Transaction<'_>,
    mut item: StoredItem,
) -> rusqlite::Result<StoredItem> {
    if item.is_sensitive {
        return Ok(item);
    }
    tx.execute(
        "UPDATE clipboard_items SET is_sensitive = 1 WHERE id = ?1 AND deleted = 0",
        [&item.id],
    )?;
    // This must share the classification update's transaction: otherwise a
    // password-manager re-copy can leave an ordinary row searchable.
    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [&item.id])?;
    item.is_sensitive = true;
    Ok(item)
}

impl Store {
    /// Stores a capture, or promotes the row that already holds this content.
    ///
    /// Dedup is **unbounded**: a match is looked for across all live history,
    /// not inside a recency window, and the survivor's `created_at` is moved to
    /// the new capture time (manifest 01 I-23 / T-36 / T-39, manifest 03 D9).
    /// Re-copying something from last week promotes it instead of growing a
    /// second row, which is the behaviour a clipboard manager is judged on.
    ///
    /// # The bump is a version stamp, not a display hint
    ///
    /// `created_at` is merge key 1 on both sync transports
    /// (`copypaste_p2p::sync::merge_decision`), so restamping it publishes a new
    /// version: the peer takes it, and the item rises on that device too. That
    /// is the intended reading of a re-copy — the user touched this item now, on
    /// this device — and it converges, because the two sides then tie on all
    /// four keys and `KeepLocal` ends it. The rejected alternative was a
    /// local-only `bumped_at` used for ordering: it needs a second sort key that
    /// the merge does not see, which is how the local list order and the synced
    /// order come apart.
    ///
    /// A bump only ever moves the stamp *forward* (T-37), and it leaves
    /// `pinned` / `pin_order` alone: a re-copied pin keeps its slot in the
    /// pinned section rather than jumping to the top (manifest 06 INV-31).
    pub fn insert_or_bump(&self, item: NewItem) -> Result<Ingest, StoreError> {
        // ADR-015 layer 1: unconditional, and it ignores what the caller
        // passed. A sensitive item is never indexed.
        let search_text =
            if item.is_sensitive || !copypaste_ipc::content_type::is_text(&item.content_type) {
                if item.search_text.is_some() {
                    tracing::warn!(
                        "search_text supplied for a sensitive item; dropping it (it must be None)"
                    );
                }
                None
            } else {
                item.search_text.as_deref().filter(|t| !t.trim().is_empty())
            };

        // The caller's id, not a fresh one: the ciphertext is already sealed
        // against it (see `NewItem::id`).
        let id = item.id.clone();
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;

        // The probe and the bump share the insert's transaction, so no third
        // capture can land between finding the row and restamping it.
        if let Some(existing) = newest_live_with_hash(&tx, &item.content_hash, i64::MIN)? {
            let existing = if item.is_sensitive {
                promote_sensitive_in_tx(&tx, existing)?
            } else {
                existing
            };
            let bumped = bump_in_tx(
                &tx,
                &existing,
                item.created_at,
                &item.app_bundle_id,
                &item.app_name,
            )?;
            tx.commit()?;
            return Ok(Ingest::Bumped(bumped));
        }

        let insert = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted, app_bundle_id, app_name, payload_metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, 0, ?8, ?9, ?10)",
            params![
                &id,
                &item.content_ciphertext,
                &item.nonce,
                &item.content_type,
                &item.content_hash,
                item.is_sensitive,
                item.created_at,
                &item.app_bundle_id,
                &item.app_name,
                &item.payload_metadata,
            ],
        );

        match insert {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                // The dedup backstop fired: a concurrent capture of the same
                // content committed between the probe above and this INSERT.
                // Resolve the winner *inside* the same transaction so there is
                // no TOCTOU gap between the failed INSERT and this lookup.
                let existing = find_in_bucket(&tx, &item.content_hash, item.created_at)?;
                return match existing {
                    Some(existing) => {
                        let existing = if item.is_sensitive {
                            promote_sensitive_in_tx(&tx, existing)?
                        } else {
                            existing
                        };
                        let bumped = bump_in_tx(
                            &tx,
                            &existing,
                            item.created_at,
                            &item.app_bundle_id,
                            &item.app_name,
                        )?;
                        tx.commit()?;
                        Ok(Ingest::Bumped(bumped))
                    }
                    None => Err(e.into()),
                };
            }
            Err(e) => return Err(e.into()),
        }

        if let Some(text) = search_text {
            upsert_fts_in_tx(&tx, &id, text)?;
        }
        tx.commit()?;

        Ok(Ingest::Inserted(StoredItem {
            id,
            content_ciphertext: item.content_ciphertext,
            nonce: item.nonce,
            content_type: item.content_type,
            content_hash: item.content_hash,
            created_at: item.created_at,
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
            is_sensitive: item.is_sensitive,
            deleted: false,
            // A capture on this device. The empty sentinel rather than a device
            // id the store has no business knowing — see `versions::origin_or`.
            origin_device_id: String::new(),
            app_bundle_id: item.app_bundle_id,
            app_name: item.app_name,
            payload_metadata: item.payload_metadata,
        }))
    }

    /// [`Store::insert_or_bump`] for callers that do not need to know which of
    /// the two happened.
    pub fn insert(&self, item: NewItem) -> Result<StoredItem, StoreError> {
        self.insert_or_bump(item).map(Ingest::into_item)
    }

    /// Raises a live row's sensitivity classification and removes its FTS
    /// entry in the same transaction.
    pub fn promote_to_sensitive(&self, item: StoredItem) -> Result<StoredItem, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let promoted = promote_sensitive_in_tx(&tx, item)?;
        tx.commit()?;
        Ok(promoted)
    }

    /// Pinned first, then newest first, by offset.
    ///
    /// The order is *total* (`pinned DESC, pin_order, created_at DESC, id DESC`)
    /// — the trailing `id` tiebreak keeps pages stable when rows tie on
    /// `created_at`, and it is what [`Store::list_from`] seeks on.
    ///
    /// Prefer `list_from` for anything a user scrolls: an offset window shifts
    /// under a list that grows at the top, which is `CopyPaste-8ebg.57`.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items WHERE deleted = 0 \
              ORDER BY pinned DESC, pin_order ASC, created_at DESC, id DESC \
              LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![i64::from(limit), i64::from(offset)], row_to_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Fetches one live item. Tombstones are not live and return `None`.
    pub fn get(&self, id: &str) -> Result<Option<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items WHERE id = ?1 AND deleted = 0"
        ))?;
        Ok(stmt.query_row([id], row_to_item).optional()?)
    }

    /// Soft-deletes an item, returning whether a live row was affected.
    ///
    /// The ciphertext and nonce are wiped and the FTS row is removed in the same
    /// transaction; the row survives as a tombstone so a later sync layer can
    /// propagate the delete instead of resurrecting the item.
    pub fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    content_hash = CASE WHEN is_sensitive = 1 THEN '' ELSE content_hash END, \
                    created_at = CASE \
                        WHEN created_at = 9223372036854775807 THEN created_at \
                        ELSE MAX(created_at + 1, ?2) \
                    END, \
                    pinned = 0, pin_order = NULL, app_bundle_id = NULL, app_name = NULL, payload_metadata = NULL \
              WHERE id = ?1 AND deleted = 0",
            params![id, crate::now_ms()],
        )?;
        // Unconditional: this also repairs a stale row left by an earlier
        // partial failure.
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Turn exactly the sensitive version inspected by the auto-wipe into a
    /// tombstone. The predicates close the select/decrypt/delete race: a pin
    /// or a re-copy after the sweep selected its candidate must keep the item.
    pub(crate) fn wipe_sensitive_if_unchanged(
        &self,
        id: &str,
        created_at: i64,
        content_hash: &str,
    ) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    content_hash = '', pinned = 0, pin_order = NULL, app_bundle_id = NULL, app_name = NULL, \
                    payload_metadata = NULL, \
                    created_at = CASE \
                        WHEN created_at = 9223372036854775807 THEN created_at \
                        ELSE MAX(created_at + 1, ?4) \
                    END \
              WHERE id = ?1 AND created_at = ?2 AND content_hash = ?3 \
                AND is_sensitive = 1 AND pinned = 0 AND deleted = 0",
            params![id, created_at, content_hash, crate::now_ms()],
        )?;
        if changed > 0 {
            tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Soft-deletes every live item, returning how many were affected.
    pub fn delete_all(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        // Pinned rows survive. Manifest 04 is explicit that delete_all
        // tombstones non-pinned rows only, and pinning is the one gesture by
        // which a user says "keep this" — clearing history must not be the
        // thing that discards it.
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    content_hash = CASE WHEN is_sensitive = 1 THEN '' ELSE content_hash END, \
                    created_at = CASE \
                        WHEN created_at = 9223372036854775807 THEN created_at \
                        ELSE MAX(created_at + 1, ?1) \
                    END, \
                    pin_order = NULL, app_bundle_id = NULL, app_name = NULL, payload_metadata = NULL \
              WHERE deleted = 0 AND pinned = 0",
            [crate::now_ms()],
        )?;
        // Drop index rows for everything that is no longer live, which also
        // sweeps orphans. Pinned rows stay live and keep their entries.
        tx.execute(
            "DELETE FROM clipboard_fts \
              WHERE id NOT IN (SELECT id FROM clipboard_items WHERE deleted = 0)",
            [],
        )?;
        tx.commit()?;
        Ok(changed as u64)
    }

    /// Number of live items. Tombstones do not count.
    pub fn count(&self) -> Result<u64, StoreError> {
        let conn = self.conn()?;
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
            [],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{fts_dump, fts_row_count, item, store, T0};

    #[test]
    fn insert_and_get_round_trip() {
        let s = store();
        let stored = s.insert(item("hello world", T0)).unwrap();

        let fetched = s.get(&stored.id).unwrap().expect("item is present");
        assert_eq!(fetched, stored);
        assert_eq!(fetched.content_ciphertext, b"ct:hello world");
        assert_eq!(fetched.nonce, vec![1u8; 24]);
        assert_eq!(fetched.content_type, "text");
        assert_eq!(fetched.created_at, T0);
        assert!(!fetched.pinned);
        assert!(!fetched.is_sensitive);

        assert!(s.get("no-such-id").unwrap().is_none());
    }

    #[test]
    fn list_is_pinned_first_then_newest_first() {
        let s = store();
        let oldest = s.insert(item("oldest", T0)).unwrap();
        let middle = s.insert(item("middle", T0 + 60_000)).unwrap();
        let newest = s.insert(item("newest", T0 + 120_000)).unwrap();

        let ids: Vec<_> = s.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            vec![newest.id.clone(), middle.id.clone(), oldest.id.clone()]
        );

        assert!(s.set_pinned(&oldest.id, true).unwrap());
        let page = s.list(10, 0).unwrap();
        assert_eq!(page[0].id, oldest.id);
        assert!(page[0].pinned);
        assert_eq!(page[1].id, newest.id);
        assert_eq!(page[2].id, middle.id);

        assert!(s.set_pinned(&middle.id, true).unwrap());
        let ids: Vec<_> = s.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![oldest.id, middle.id, newest.id]);

        let s2 = store();
        for n in 0..5 {
            s2.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                .unwrap();
        }
        let all: Vec<_> = s2.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        let page2: Vec<_> = s2.list(2, 2).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(page2, all[2..4].to_vec());
    }

    #[test]
    fn delete_tombstones_the_row_and_clears_the_index() {
        let s = store();
        let a = s.insert(item("first payload", T0)).unwrap();
        let b = s.insert(item("second payload", T0 + 60_000)).unwrap();

        assert!(s.delete(&a.id).unwrap());
        assert!(s.get(&a.id).unwrap().is_none());
        assert_eq!(s.count().unwrap(), 1);
        assert_eq!(fts_row_count(&s, &a.id), 0);
        assert!(s.search("first", 10).unwrap().is_empty());
        assert_eq!(s.list(10, 0).unwrap().len(), 1);

        // The tombstone survives with its payload wiped.
        {
            let conn = s.conn().unwrap();
            let (deleted, ct): (bool, Option<Vec<u8>>) = conn
                .query_row(
                    "SELECT deleted, content_ciphertext FROM clipboard_items WHERE id = ?1",
                    [&a.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert!(deleted);
            assert!(ct.is_none());
        }

        assert!(!s.delete(&a.id).unwrap());
        assert!(!s.delete("no-such-id").unwrap());
        assert!(s.get(&b.id).unwrap().is_some());
    }

    #[test]
    fn delete_all_clears_every_live_item() {
        let s = store();
        for n in 0..4 {
            s.insert(item(&format!("payload {n}"), T0 + n * 60_000))
                .unwrap();
        }
        assert_eq!(s.count().unwrap(), 4);

        assert_eq!(s.delete_all().unwrap(), 4);
        assert_eq!(s.count().unwrap(), 0);
        assert!(s.list(10, 0).unwrap().is_empty());
        assert!(s.search("payload", 10).unwrap().is_empty());
        assert_eq!(fts_dump(&s), "");

        assert_eq!(s.delete_all().unwrap(), 0);
    }

    /// Manifest 04: `delete_all` tombstones non-pinned rows only. The
    /// regression was silent — every other test passed with pinned rows being
    /// wiped, because none of them pinned anything first.
    #[test]
    fn delete_all_leaves_pinned_items_intact() {
        let s = store();
        let keep = s.insert(item("pinned payload", T0)).unwrap();
        for n in 1..4 {
            s.insert(item(&format!("payload {n}"), T0 + n * 60_000))
                .unwrap();
        }
        assert!(s.set_pinned(&keep.id, true).unwrap());

        assert_eq!(s.delete_all().unwrap(), 3);
        assert_eq!(s.count().unwrap(), 1);

        let left = s.list(10, 0).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, keep.id);
        assert!(left[0].pinned, "the survivor must still be pinned");
        assert!(
            s.get(&keep.id).unwrap().is_some(),
            "a pinned item must survive delete_all"
        );

        // It keeps its search entry; the cleared rows lose theirs.
        assert_eq!(s.search("pinned", 10).unwrap().len(), 1);
        assert!(s.search("payload 2", 10).unwrap().is_empty());

        // A second call is a no-op rather than finally taking the pinned row.
        assert_eq!(s.delete_all().unwrap(), 0);
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn set_pinned_toggles_and_reports_existence() {
        let s = store();
        let a = s.insert(item("pin me", T0)).unwrap();

        assert!(s.set_pinned(&a.id, true).unwrap());
        assert!(s.get(&a.id).unwrap().unwrap().pinned);
        assert!(s.set_pinned(&a.id, true).unwrap());
        assert!(s.get(&a.id).unwrap().unwrap().pinned);

        assert!(s.set_pinned(&a.id, false).unwrap());
        assert!(!s.get(&a.id).unwrap().unwrap().pinned);

        assert!(!s.set_pinned("no-such-id", true).unwrap());

        s.delete(&a.id).unwrap();
        assert!(!s.set_pinned(&a.id, true).unwrap());
    }

    #[test]
    fn count_tracks_live_items_only() {
        let s = store();
        assert_eq!(s.count().unwrap(), 0);
        let a = s.insert(item("one", T0)).unwrap();
        s.insert(item("two", T0 + 60_000)).unwrap();
        assert_eq!(s.count().unwrap(), 2);
        s.delete(&a.id).unwrap();
        assert_eq!(s.count().unwrap(), 1);
    }
}
