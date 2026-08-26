//! Item CRUD: everything that writes or reads a `clipboard_items` row without
//! going through FTS or a retention sweep. Two invariants live here: layer 1 of
//! the sensitive/FTS exclusion (in [`Store::insert_or_bump`]), and that
//! a delete is a *tombstone*, not a row removal.

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::{
    is_constraint_violation, item_columns, row_to_item, Ingest, ItemColumns, NewItem, StoreError,
    StoredItem,
};
use super::retention::{bump_in_tx, find_in_bucket, live_count, newest_live_with_hash};
use super::search::{delete_fts_row_in_tx, insert_fts_in_tx};
use super::store::Store;

fn promote_sensitive_in_tx(
    tx: &rusqlite::Transaction<'_>,
    mut item: StoredItem,
) -> rusqlite::Result<StoredItem> {
    if item.is_sensitive {
        return Ok(item);
    }
    // This must share the classification update's transaction: otherwise a
    // password-manager re-copy can leave an ordinary row searchable. The index
    // row goes first, while `fts_rowid` still names it.
    delete_fts_row_in_tx(tx, &item.id)?;
    tx.execute(
        "UPDATE clipboard_items SET is_sensitive = 1, fts_rowid = NULL \
          WHERE id = ?1 AND deleted = 0",
        [&item.id],
    )?;
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
    pub fn insert_or_bump(&self, mut item: NewItem) -> Result<Ingest, StoreError> {
        let sealed = (
            std::mem::take(&mut item.nonce),
            std::mem::take(&mut item.content_ciphertext),
            item.search_text.take(),
        );
        self.insert_or_bump_late_sealed(item, move || Ok(sealed))
    }

    pub(crate) fn insert_or_bump_late_sealed<E, F>(
        &self,
        item: NewItem,
        seal: F,
    ) -> Result<Ingest, E>
    where
        E: From<StoreError>,
        F: FnOnce() -> Result<(Vec<u8>, Vec<u8>, Option<String>), E>,
    {
        // `CopyPaste-i6pp` layer 1: unconditional, and it ignores what the caller
        // passed. A sensitive item is never indexed.
        let indexable =
            !item.is_sensitive && copypaste_ipc::content_type::is_text(&item.content_type);

        // The caller's id, not a fresh one: the ciphertext is already sealed
        // against it (see `NewItem::id`).
        let id = item.id.clone();
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn).map_err(StoreError::from)?;

        // The probe and the bump share the insert's transaction, so no third
        // capture can land between finding the row and restamping it.
        if let Some(existing) =
            newest_live_with_hash(&tx, &item.content_hash, i64::MIN).map_err(StoreError::from)?
        {
            let existing = if item.is_sensitive {
                promote_sensitive_in_tx(&tx, existing).map_err(StoreError::from)?
            } else {
                existing
            };
            let bumped = bump_in_tx(
                &tx,
                &existing,
                item.created_at,
                &item.app_bundle_id,
                &item.app_name,
            )
            .map_err(StoreError::from)?;
            tx.commit().map_err(StoreError::from)?;
            return Ok(Ingest::Bumped(bumped));
        }

        let (nonce, content_ciphertext, plaintext) = seal()?;
        let search_text = plaintext
            .as_deref()
            .filter(|t| indexable && !t.trim().is_empty());
        if !indexable && plaintext.is_some() {
            tracing::warn!(
                "search_text supplied for a sensitive item; dropping it (it must be None)"
            );
        }

        let fts_rowid = match search_text {
            Some(text) => insert_fts_in_tx(&tx, &id, text, item.is_sensitive, &item.content_type)
                .map_err(StoreError::from)?,
            None => None,
        };

        let insert = tx.execute(
            // `content_bytes` is computed in this statement rather than by a
            // trigger, which would have to UPDATE the row and write the whole
            // payload to the WAL a second time.
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted, app_bundle_id, app_name, payload_metadata, \
                  fts_rowid, content_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, 0, ?8, ?9, ?10, ?11, \
                     LENGTH(COALESCE(?2, X'')))",
            params![
                &id,
                &content_ciphertext,
                &nonce,
                &item.content_type,
                &item.content_hash,
                item.is_sensitive,
                item.created_at,
                &item.app_bundle_id,
                &item.app_name,
                &item.payload_metadata,
                fts_rowid,
            ],
        );

        match insert {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                if let Some(rowid) = fts_rowid {
                    tx.execute("DELETE FROM clipboard_fts WHERE rowid = ?1", [rowid])
                        .map_err(StoreError::from)?;
                }
                // The dedup backstop fired: a concurrent capture of the same
                // content committed between the probe above and this INSERT.
                // Resolve the winner *inside* the same transaction so there is
                // no TOCTOU gap between the failed INSERT and this lookup.
                let existing = find_in_bucket(&tx, &item.content_hash, item.created_at)
                    .map_err(StoreError::from)?;
                return match existing {
                    Some(existing) => {
                        let existing = if item.is_sensitive {
                            promote_sensitive_in_tx(&tx, existing).map_err(StoreError::from)?
                        } else {
                            existing
                        };
                        let bumped = bump_in_tx(
                            &tx,
                            &existing,
                            item.created_at,
                            &item.app_bundle_id,
                            &item.app_name,
                        )
                        .map_err(StoreError::from)?;
                        tx.commit().map_err(StoreError::from)?;
                        Ok(Ingest::Bumped(bumped))
                    }
                    None => Err(StoreError::from(e).into()),
                };
            }
            Err(e) => return Err(StoreError::from(e).into()),
        }

        tx.commit().map_err(StoreError::from)?;

        Ok(Ingest::Inserted(StoredItem {
            id,
            content_ciphertext,
            nonce,
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
        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map(params![i64::from(limit), i64::from(offset)], |row| {
            row_to_item(row, &columns)
        })?;
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
        let columns = ItemColumns::resolve(&stmt)?;
        Ok(stmt
            .query_row([id], |row| row_to_item(row, &columns))
            .optional()?)
    }

    /// Soft-deletes an item, returning whether a live row was affected.
    ///
    /// The ciphertext and nonce are wiped and the FTS row is removed in the same
    /// transaction; the row survives as a tombstone so a later sync layer can
    /// propagate the delete instead of resurrecting the item.
    pub fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        // Unconditional, and before the update that clears `fts_rowid`.
        delete_fts_row_in_tx(&tx, id)?;
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, content_bytes = 0, nonce = NULL, \
                    content_hash = CASE WHEN is_sensitive = 1 THEN '' ELSE content_hash END, \
                    created_at = CASE \
                        WHEN created_at = 9223372036854775807 THEN created_at \
                        ELSE MAX(created_at + 1, ?2) \
                    END, \
                    pinned = 0, pin_order = NULL, app_bundle_id = NULL, app_name = NULL, payload_metadata = NULL, \
                    fts_rowid = NULL \
              WHERE id = ?1 AND deleted = 0",
            params![id, crate::now_ms()],
        )?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Turn exactly the sensitive version inspected by the auto-wipe into a
    /// tombstone. The predicates close the select/decrypt/delete race: a pin
    /// or a re-copy after the sweep selected its candidate must keep the item.
    #[cfg(test)]
    pub(crate) fn wipe_sensitive_if_unchanged(
        &self,
        id: &str,
        created_at: i64,
        content_hash: &str,
    ) -> Result<bool, StoreError> {
        Ok(self.wipe_sensitive_batch_if_unchanged(&[(
            id.to_string(),
            created_at,
            content_hash.to_string(),
        )])? > 0)
    }

    pub(crate) fn wipe_sensitive_batch_if_unchanged(
        &self,
        victims: &[(String, i64, String)],
    ) -> Result<u64, StoreError> {
        if victims.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let now = crate::now_ms();
        let mut removed = 0u64;
        for (id, created_at, content_hash) in victims {
            let fts_rowid: Option<i64> = tx
                .query_row(
                    "SELECT fts_rowid FROM clipboard_items WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            let changed = tx.execute(
                "UPDATE clipboard_items                     SET deleted = 1, content_ciphertext = NULL, content_bytes = 0, nonce = NULL,                         content_hash = '', pinned = 0, pin_order = NULL, app_bundle_id = NULL, app_name = NULL,                         payload_metadata = NULL, fts_rowid = NULL,                         created_at = CASE                             WHEN created_at = 9223372036854775807 THEN created_at                             ELSE MAX(created_at + 1, ?4)                         END                   WHERE id = ?1 AND created_at = ?2 AND content_hash = ?3                     AND is_sensitive = 1 AND pinned = 0 AND deleted = 0",
                params![id, created_at, content_hash, now],
            )?;
            if let (true, Some(rowid)) = (changed > 0, fts_rowid) {
                tx.execute("DELETE FROM clipboard_fts WHERE rowid = ?1", [rowid])?;
            }
            removed += u64::from(changed > 0);
        }
        tx.commit()?;
        Ok(removed)
    }

    /// Soft-deletes every live item, returning how many were affected.
    pub fn delete_all(&self) -> Result<u64, StoreError> {
        self.delete_all_through(i64::MAX)
    }

    /// Highest `rowid` in the table, or 0 when it is empty.
    pub fn max_rowid(&self) -> Result<i64, StoreError> {
        let conn = self.conn()?;
        Ok(conn.query_row(
            "SELECT COALESCE(MAX(rowid), 0) FROM clipboard_items",
            [],
            |row| row.get(0),
        )?)
    }

    /// Soft-deletes every live item stored at or before `through`, returning how
    /// many were affected.
    ///
    /// `through` is a `rowid` and deliberately not a `created_at`. The UI defers
    /// this call behind an undo window, so the bound has to mean "what existed
    /// when the user pressed clear". A clip captured during that window and a
    /// row whose clock is skewed ahead (DMY-180) both carry a `created_at`
    /// above any press-time instant, so a timestamp cannot separate them and a
    /// skewed row would survive a clear the user believes emptied everything.
    /// `rowid` is insert order, so it separates them without consulting a clock.
    ///
    /// SQLite only reuses a `rowid` when the row holding the largest one is hard
    /// deleted. Of the three eviction paths, `evict_over_cap` and
    /// `evict_over_byte_cap` take the oldest, but `evict_older_than` selects on
    /// `created_at`, so a clock jump **backward** can backdate the newest row —
    /// the one holding the largest `rowid` — into the age sweep. Reuse then
    /// needs a fresh insert inside the same undo window to land at or below a
    /// ceiling already taken, which would clear a clip captured after the user
    /// pressed. Narrow, and not closed by this bound.
    ///
    /// Repeated calls need no guard. `deleted = 0` excludes anything an earlier
    /// call tombstoned, and a later ceiling is strictly larger, so it can only
    /// add rows that are still live.
    pub fn delete_all_through(&self, through: i64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        // Pinned rows survive. Manifest 04 is explicit that delete_all
        // tombstones non-pinned rows only, and pinning is the one gesture by
        // which a user says "keep this" — clearing history must not be the
        // thing that discards it.
        //
        // 9223372036854775807 is i64::MAX used as a saturation ceiling
        // (manifest 03 §3.11), so the CASE is overflow safety for a row already
        // at the ceiling — not a sentinel marking a row to treat differently.
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, content_bytes = 0, nonce = NULL, \
                    content_hash = CASE WHEN is_sensitive = 1 THEN '' ELSE content_hash END, \
                    created_at = CASE \
                        WHEN created_at = 9223372036854775807 THEN created_at \
                        ELSE MAX(created_at + 1, ?1) \
                    END, \
                    pin_order = NULL, app_bundle_id = NULL, app_name = NULL, payload_metadata = NULL, \
                    fts_rowid = NULL \
              WHERE deleted = 0 AND pinned = 0 AND rowid <= ?2",
            params![crate::now_ms(), through],
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
        Ok(live_count(&conn)?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::NewItem;
    use super::super::test_support::{
        fts_dump, fts_row_count, item, plant_fts_row, sensitive_item, store, KEY, T0,
    };
    use super::super::Store;

    #[test]
    fn a_capture_writes_its_payload_to_the_wal_once() {
        const PAYLOAD: usize = 1 << 20;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let wal = std::path::PathBuf::from(format!("{}-wal", path.display()));
        let s = Store::open(&path, &KEY).unwrap();

        let big = |n: i64| NewItem {
            content_ciphertext: vec![b'x'; PAYLOAD],
            search_text: Some(format!("marker {n}")),
            ..item(&format!("payload {n}"), T0 + n * 60_000)
        };

        s.insert(big(0)).unwrap();
        {
            let conn = s.conn().unwrap();
            super::super::connection::run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        }
        assert!(std::fs::metadata(&wal).unwrap().len() < PAYLOAD as u64 / 8);

        s.insert(big(1)).unwrap();
        let written = std::fs::metadata(&wal).unwrap().len();
        assert!(
            written < PAYLOAD as u64 * 3 / 2,
            "one {PAYLOAD}-byte capture put {written} bytes in the WAL; \
             the payload is being written twice"
        );
    }

    #[test]
    fn a_capture_the_insert_rejects_leaves_no_plaintext_in_the_index() {
        let s = store();
        let first = s.insert(item("alpha payload", T0)).unwrap();

        let clash = NewItem {
            id: first.id.clone(),
            ..item("beta payload leaking", T0 + 60_000)
        };
        assert!(s.insert(clash).is_err());

        assert_eq!(fts_row_count(&s, &first.id), 1);
        assert!(!fts_dump(&s).contains("leaking"));
        assert!(s.search("leaking", 10).unwrap().is_empty());
        assert_eq!(s.search("alpha", 10).unwrap().len(), 1);
    }

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
    fn delete_restamps_created_at_above_the_live_version() {
        let s = store();
        let a = s.insert(item("payload", T0)).unwrap();
        assert!(s.delete(&a.id).unwrap());
        let tomb = s.version(&a.id).unwrap().unwrap();
        assert!(tomb.deleted);
        assert!(tomb.created_at > T0);
        assert!(tomb.created_at >= T0.saturating_add(1));
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

    /// The UI defers `delete_all` behind an undo window, so a clip captured
    /// during that window must survive an action the user took before it
    /// existed.
    #[test]
    fn delete_all_through_spares_what_arrived_after_the_bound() {
        let s = store();
        for n in 0..3 {
            s.insert(item(&format!("before {n}"), T0 + n * 60_000))
                .unwrap();
        }
        let bound = s.max_rowid().unwrap();
        let after = s.insert(item("captured mid-window", T0 + 600_000)).unwrap();

        assert_eq!(s.delete_all_through(bound).unwrap(), 3);
        let live = s.list(10, 0).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, after.id);
    }

    /// A deferred clear must remove exactly what an immediate one would. Both a
    /// clock skewed ahead (DMY-180) and the `i64::MAX` saturation ceiling put
    /// `created_at` above any press-time instant, which is why the bound is a
    /// `rowid` — a timestamp bound would leave these behind.
    #[test]
    fn delete_all_through_clears_rows_dated_ahead_of_the_clock() {
        let s = store();
        s.insert(item("skewed", crate::now_ms() + 86_400_000))
            .unwrap();
        s.insert(item("at the ceiling", i64::MAX)).unwrap();
        let bound = s.max_rowid().unwrap();

        assert_eq!(s.delete_all_through(bound).unwrap(), 2);
        assert_eq!(s.count().unwrap(), 0);
        assert_eq!(fts_dump(&s), "");
    }

    /// A sensitive item is out of the index before the clear, is not put into it
    /// by the clear, and is still out of it afterwards while it remains live —
    /// the state an undone clear leaves behind.
    #[test]
    fn a_sensitive_item_spared_by_the_bound_never_enters_the_index() {
        let s = store();
        let doomed = s.insert(item("ordinary", T0)).unwrap();
        let bound = s.max_rowid().unwrap();
        let secret = s.insert(sensitive_item("swordfish", T0 + 1_000)).unwrap();

        assert_eq!(fts_row_count(&s, &secret.id), 0);
        assert_eq!(s.delete_all_through(bound).unwrap(), 1);

        assert_eq!(fts_row_count(&s, &secret.id), 0);
        assert_eq!(fts_row_count(&s, &doomed.id), 0);
        assert!(s.search("swordfish", 10).unwrap().is_empty());
        assert_eq!(s.count().unwrap(), 1);
    }

    /// A database written before the write-time guard existed carries the index
    /// row anyway; clearing must sweep it rather than leave it searchable.
    #[test]
    fn a_planted_index_row_does_not_survive_the_clear() {
        let s = store();
        let secret = s.insert(sensitive_item("swordfish", T0)).unwrap();
        plant_fts_row(&s, &secret.id, "swordfish");
        assert_eq!(fts_row_count(&s, &secret.id), 1);

        assert_eq!(s.delete_all_through(s.max_rowid().unwrap()).unwrap(), 1);
        assert_eq!(fts_row_count(&s, &secret.id), 0);
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
