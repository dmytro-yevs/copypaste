//! Pinning, and the order pinned items are shown in.
//!
//! `pin_order` is `REAL` and `NULL` on unpinned rows; the list order is
//! `pinned DESC, pin_order ASC, created_at DESC, id DESC`. Both writers here
//! keep that column consistent with `pinned`, which is what the partial indexes
//! in the schema assume.

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::StoreError;
use super::store::Store;

impl Store {
    /// Pins or unpins an item. Returns `false` if there is no such live item.
    ///
    /// Pinning is idempotent; a newly pinned item lands at the end of the pinned
    /// section.
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
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

    /// Sets the pinned ordering to exactly `ids`, in the order given, and
    /// returns how many rows were renumbered.
    ///
    /// The whole order rather than a move-one verb: a partial move is ambiguous
    /// the moment a peer pins something between the read and the write, and the
    /// client already holds the list it is reordering.
    ///
    /// * Ids that are absent, deleted or not pinned are **ignored**, not an
    ///   error — a sync round may legitimately have removed one since the
    ///   client read the list, and failing the whole gesture over it would lose
    ///   the reorder the user just made.
    /// * Pinned rows the caller did not name keep their relative order and sort
    ///   *after* the named ones, so a stale client cannot silently reshuffle a
    ///   pin it never saw.
    ///
    /// One IMMEDIATE transaction, per [`write_tx`]: this reads the pinned set
    /// and then writes it, which is exactly the read-then-write that returns
    /// `SQLITE_BUSY_SNAPSHOT` on a DEFERRED transaction while another
    /// connection is writing.
    pub fn reorder_pinned(&self, ids: &[String]) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;

        // The pinned set as it stands now, in its current order. Read inside
        // the transaction so the unnamed tail cannot move underneath us.
        let current: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM clipboard_items \
                  WHERE pinned = 1 AND deleted = 0 \
                  ORDER BY pin_order ASC, created_at DESC, id DESC",
            )?;
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row.get(0)?);
            }
            out
        };

        // Named-and-still-pinned first, in the caller's order, deduplicated;
        // then everything else, in the order it already had.
        let mut ordered: Vec<&String> = Vec::with_capacity(current.len());
        for id in ids {
            if current.contains(id) && !ordered.contains(&id) {
                ordered.push(id);
            }
        }
        let named = ordered.len();
        for id in &current {
            if !ordered.contains(&id) {
                ordered.push(id);
            }
        }

        // 1.0 upward, matching what `set_pinned` assumes when it appends at
        // `MAX(pin_order) + 1`.
        let mut renumbered = 0u64;
        {
            let mut stmt = tx.prepare(
                "UPDATE clipboard_items SET pin_order = ?2 \
                  WHERE id = ?1 AND pinned = 1 AND deleted = 0",
            )?;
            for (index, id) in ordered.iter().enumerate() {
                renumbered += stmt.execute(params![id, (index + 1) as f64])? as u64;
            }
        }
        tx.commit()?;

        tracing::debug!(named, renumbered, "reordered the pinned items");
        Ok(renumbered)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};

    fn pinned_order(s: &super::Store) -> Vec<String> {
        s.list(50, 0)
            .unwrap()
            .into_iter()
            .filter(|row| row.pinned)
            .map(|row| row.id)
            .collect()
    }

    /// Three items, pinned oldest-first, then reversed.
    fn three_pinned() -> (super::Store, Vec<String>) {
        let s = store();
        let ids: Vec<String> = (0..3)
            .map(|n| {
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap()
                    .id
            })
            .collect();
        for id in &ids {
            assert!(s.set_pinned(id, true).unwrap());
        }
        assert_eq!(pinned_order(&s), ids);
        (s, ids)
    }

    #[test]
    fn a_full_reorder_takes_the_order_given() {
        let (s, ids) = three_pinned();
        let reversed: Vec<String> = ids.iter().rev().cloned().collect();

        assert_eq!(s.reorder_pinned(&reversed).unwrap(), 3);
        assert_eq!(pinned_order(&s), reversed);

        // Idempotent: applying the same order again is a no-op to the result.
        assert_eq!(s.reorder_pinned(&reversed).unwrap(), 3);
        assert_eq!(pinned_order(&s), reversed);
    }

    /// A stale client naming only part of the list must not reshuffle the rest.
    #[test]
    fn unnamed_pins_keep_their_relative_order_and_sort_after_the_named_ones() {
        let (s, ids) = three_pinned();
        assert_eq!(s.reorder_pinned(&[ids[2].clone()]).unwrap(), 3);
        assert_eq!(
            pinned_order(&s),
            [&ids[2], &ids[0], &ids[1]].map(String::from)
        );
    }

    /// A peer may have deleted or unpinned one between the read and the write.
    /// Dropping the whole gesture over that would lose the user's reorder.
    #[test]
    fn unknown_unpinned_and_repeated_ids_are_ignored_rather_than_refused() {
        let (s, ids) = three_pinned();
        let unpinned = s.insert(item("not pinned", T0 + 999_000)).unwrap().id;

        let requested = vec![
            "no-such-item".to_string(),
            unpinned.clone(),
            ids[1].clone(),
            ids[1].clone(),
        ];
        assert_eq!(s.reorder_pinned(&requested).unwrap(), 3);
        assert_eq!(
            pinned_order(&s),
            [&ids[1], &ids[0], &ids[2]].map(String::from)
        );
        assert!(!s.get(&unpinned).unwrap().unwrap().pinned);
    }

    #[test]
    fn reordering_with_nothing_pinned_is_a_no_op() {
        let s = store();
        s.insert(item("loose", T0)).unwrap();
        assert_eq!(s.reorder_pinned(&["loose".to_string()]).unwrap(), 0);
        assert_eq!(s.reorder_pinned(&[]).unwrap(), 0);
    }

    /// `set_pinned` appends at `MAX(pin_order) + 1`, so a renumbered list must
    /// leave room for the next pin at the end rather than in the middle.
    #[test]
    fn a_pin_added_after_a_reorder_lands_at_the_end() {
        let (s, ids) = three_pinned();
        s.reorder_pinned(&ids.iter().rev().cloned().collect::<Vec<_>>())
            .unwrap();

        let latecomer = s.insert(item("latecomer", T0 + 500_000)).unwrap().id;
        assert!(s.set_pinned(&latecomer, true).unwrap());

        let order = pinned_order(&s);
        assert_eq!(order.last().unwrap(), &latecomer);
        assert_eq!(order.len(), 4);
    }
}
