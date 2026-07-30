//! Item CRUD: everything that writes or reads a `clipboard_items` row without
//! going through FTS or a retention sweep. Two invariants live here: layer 1 of
//! the ADR-015 sensitive/FTS exclusion (in [`Store::insert`]), and that a
//! delete is a *tombstone*, not a row removal.

use rusqlite::{params, OptionalExtension};

use super::model::{
    is_constraint_violation, item_columns, row_to_item, NewItem, StoreError, StoredItem,
};
use super::retention::find_in_bucket;
use super::search::upsert_fts_in_tx;
use super::store::Store;

impl Store {
    /// Inserts an item and returns it as stored.
    ///
    /// Dedup: if an identical `content_hash` already occupies the same dedup
    /// bucket, no new row is written and the existing item is returned, so the
    /// call is idempotent under a race.
    pub fn insert(&self, item: NewItem) -> Result<StoredItem, StoreError> {
        // ADR-015 layer 1: unconditional, and it ignores what the caller
        // passed. A sensitive item is never indexed.
        let search_text = if item.is_sensitive {
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
        let tx = conn.transaction()?;

        let insert = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, 0)",
            params![
                &id,
                &item.content_ciphertext,
                &item.nonce,
                &item.content_type,
                &item.content_hash,
                item.is_sensitive,
                item.created_at,
            ],
        );

        match insert {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                // The dedup backstop fired. Resolve the winner *inside* the same
                // transaction so there is no TOCTOU gap between the failed
                // INSERT and this lookup.
                let existing = find_in_bucket(&tx, &item.content_hash, item.created_at)?;
                return match existing {
                    Some(existing) => {
                        tx.rollback()?;
                        Ok(existing)
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

        Ok(StoredItem {
            id,
            content_ciphertext: item.content_ciphertext,
            nonce: item.nonce,
            content_type: item.content_type,
            created_at: item.created_at,
            pinned: false,
            is_sensitive: item.is_sensitive,
        })
    }

    /// Pinned first, then newest first.
    ///
    /// The order is *total* (`pinned DESC, pin_order, created_at DESC, id DESC`)
    /// — the trailing `id` tiebreak is what a keyset cursor would seek on, and
    /// it keeps offset pages stable when rows tie on `created_at`.
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
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    pinned = 0, pin_order = NULL \
              WHERE id = ?1 AND deleted = 0",
            [id],
        )?;
        // Unconditional: this also repairs a stale row left by an earlier
        // partial failure.
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Soft-deletes every live item, returning how many were affected.
    pub fn delete_all(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        // Pinned rows survive. Manifest 04 is explicit that delete_all
        // tombstones non-pinned rows only, and pinning is the one gesture by
        // which a user says "keep this" — clearing history must not be the
        // thing that discards it.
        let changed = tx.execute(
            "UPDATE clipboard_items \
                SET deleted = 1, content_ciphertext = NULL, nonce = NULL, \
                    pin_order = NULL \
              WHERE deleted = 0 AND pinned = 0",
            [],
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

    /// Pins or unpins an item. Returns `false` if there is no such live item.
    ///
    /// Pinning is idempotent; a newly pinned item lands at the end of the pinned
    /// section.
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let exists: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM clipboard_items WHERE id = ?1 AND deleted = 0",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(false);
        }
        if pinned {
            tx.execute(
                "UPDATE clipboard_items \
                    SET pinned = 1, \
                        pin_order = (SELECT COALESCE(MAX(pin_order), 0) + 1 \
                                       FROM clipboard_items \
                                      WHERE pinned = 1 AND deleted = 0) \
                  WHERE id = ?1 AND deleted = 0 AND pinned = 0",
                [id],
            )?;
        } else {
            tx.execute(
                "UPDATE clipboard_items SET pinned = 0, pin_order = NULL \
                  WHERE id = ?1 AND deleted = 0",
                [id],
            )?;
        }
        tx.commit()?;
        Ok(true)
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
