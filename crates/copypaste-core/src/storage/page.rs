//! Keyset (seek) pagination over the history list.
//!
//! `LIMIT/OFFSET` is wrong for a list that grows at the end a user is reading
//! from: a row inserted above the window shifts everything down, so the next
//! page repeats a row it already showed or skips one it never did. That is
//! `CopyPaste-8ebg.57`, and a clipboard manager inserts above the window every
//! time the user copies anything.
//!
//! The fix needs a *total* order to seek on, which [`Store::list`] already has:
//! `pinned DESC, pin_order ASC, created_at DESC, id DESC`. What is added here is
//! the predicate that says "strictly after this row in that order", expanded one
//! OR-branch per key (manifest 03 §3.12).
//!
//! # Two segments, not one predicate
//!
//! The order concatenates two runs — pinned rows by `pin_order`, then unpinned
//! rows by recency — and `pinned` is what says which one a row is in. A page
//! therefore resumes in exactly one of them, and each is seeked separately:
//!
//! * inside the pinned run, the boolean-OR keyset expansion of manifest 03
//!   §3.12, narrowed to `pinned = 1` so the equality fixes the index prefix and
//!   `pin_order` becomes a range;
//! * inside the unpinned run, a `created_at` range on `idx_items_evictable`,
//!   whose partial predicate is `deleted = 0 AND pinned = 0` exactly.
//!
//! A page anchored in the pinned run tops up from the head of the unpinned one,
//! which is where that run starts by definition.
//!
//! Writing it as one predicate over both runs is what made this depth-linear:
//! the leading `?1 = 0 OR …` that selected "first page" left nothing sargable,
//! so every page scanned `idx_items_history` from the top and filtered. Deep
//! pages cost the whole history before them.
//!
//! # Why the branches, and not a row-value comparison
//!
//! SQLite can compare `(a, b, c) < (?, ?, ?)` in one go, but a row-value
//! comparison assumes every column runs in the same direction, and this order
//! mixes DESC and ASC. The expansion below is the portable construction, with
//! `>` on the ASC key and `<` on the DESC ones.
//!
//! `pin_order` is compared with SQLite's null-safe `IS` because a pinned row
//! whose `pin_order` is NULL would otherwise drop the tie-break branch and end
//! pagination one page early.
//!
//! The unpinned seek deliberately does **not** also require `pin_order IS
//! NULL`. That invariant holds for every row either writer in `pinning`
//! produces, but a merge writes `pinned` and `pin_order` together from a peer,
//! and a seek that assumed it would drop a row that broke it out of the list
//! entirely. `pinned` alone decides the run, so every live row is reachable.
//!
//! The cost is that such a row sorts by recency here while `Store::list` sorts
//! a non-NULL `pin_order` last. It is listed once either way.

use rusqlite::{params, OptionalExtension};

use super::model::{item_columns, row_to_item, ItemColumns, StoreError, StoredItem};
use super::store::Store;

/// Where a page stopped: the sort key of its last row.
///
/// Opaque by intent — it is a position in an ordering, not an API. Callers that
/// have to move one across a process boundary use [`ItemCursor::token`] and
/// [`ItemCursor::parse`] rather than reading the fields, so the ordering can
/// gain a key without breaking a client.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemCursor {
    pinned: bool,
    pin_order: Option<f64>,
    created_at: i64,
    id: String,
}

impl ItemCursor {
    /// The cursor as one opaque string, for a wire protocol that carries no
    /// structure of its own.
    ///
    /// Hex of the JSON: two dependencies the crate already has, and the result
    /// is inert — it survives a JSON string, a URL and a shell argument without
    /// quoting, and nothing about it invites a client to parse it.
    #[must_use]
    pub fn token(&self) -> String {
        hex::encode(serde_json::to_vec(self).unwrap_or_default())
    }

    /// Reads back a [`ItemCursor::token`].
    ///
    /// # Errors
    ///
    /// [`StoreError::InvalidCursor`] if the token is not one this build wrote.
    /// Refusing is deliberate: silently restarting from the top would make a
    /// load-more repeat the whole history.
    pub fn parse(token: &str) -> Result<Self, StoreError> {
        let bytes = hex::decode(token).map_err(|_| StoreError::InvalidCursor)?;
        serde_json::from_slice(&bytes).map_err(|_| StoreError::InvalidCursor)
    }
}

/// One page and where to resume.
#[derive(Debug, Clone)]
pub struct Page {
    pub items: Vec<StoredItem>,
    /// `None` when this page reached the end of the list.
    pub next: Option<ItemCursor>,
}

/// A row and the `pin_order` the cursor needs, which is not part of a
/// [`StoredItem`].
type Row = (StoredItem, Option<f64>);

fn fetch(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    out: &mut Vec<Row>,
) -> Result<(), StoreError> {
    let mut stmt = conn.prepare_cached(sql)?;
    let columns = ItemColumns::resolve(&stmt)?;
    let rows = stmt.query_map(params, |row| {
        Ok((
            row_to_item(row, &columns)?,
            row.get::<_, Option<f64>>(columns.pin_order_index())?,
        ))
    })?;
    for row in rows {
        out.push(row?);
    }
    Ok(())
}

/// How one seek query ended while filling a byte-bounded page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundedFetch {
    Exhausted,
    CountReached,
    BudgetReached,
}

/// Read only the rows needed to allocate a bounded page.
///
/// One discarded row proves whether a count or byte limit has more history.
/// It is mapped once, then discarded immediately without materializing later
/// rows that belong to a following page.
fn fetch_bounded(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    limit: usize,
    budget: usize,
    bytes: &mut usize,
    out: &mut Vec<Row>,
) -> Result<BoundedFetch, StoreError> {
    let mut stmt = conn.prepare_cached(sql)?;
    let columns = ItemColumns::resolve(&stmt)?;
    let mut query = stmt.query(params)?;
    loop {
        let Some(row) = query.next()? else {
            return Ok(BoundedFetch::Exhausted);
        };
        let row = (
            row_to_item(row, &columns)?,
            row.get::<_, Option<f64>>(columns.pin_order_index())?,
        );
        if out.len() == limit {
            return Ok(BoundedFetch::CountReached);
        }
        let next_bytes = bytes.saturating_add(row.0.content_ciphertext.len());
        if !out.is_empty() && next_bytes > budget {
            return Ok(BoundedFetch::BudgetReached);
        }
        *bytes = next_bytes;
        out.push(row);
    }
}

fn cursor_of((item, pin_order): &Row) -> ItemCursor {
    ItemCursor {
        pinned: item.pinned,
        pin_order: *pin_order,
        created_at: item.created_at,
        id: item.id.clone(),
    }
}

const PINNED_HEAD_SQL: &str = concat!(
    "SELECT ",
    item_columns!(),
    ", pin_order FROM clipboard_items \
      WHERE deleted = 0 AND pinned = 1 \
      ORDER BY pin_order ASC, created_at DESC, id DESC \
      LIMIT ?1"
);

/// Manifest 03 §3.12's expansion, unchanged, under a constant `pinned = 1`.
///
/// That equality fixes the leading column of `idx_items_history`, so the scan is
/// bounded by how many items are pinned rather than by how deep the history is.
/// The first branch carries `?1 IS NULL OR` because NULL sorts first in an ASC
/// order: a pinned row that somehow has no `pin_order` precedes every row that
/// has one, and everything after it must still be reachable.
const PINNED_SEEK_SQL: &str = concat!(
    "SELECT ",
    item_columns!(),
    ", pin_order FROM clipboard_items \
      WHERE deleted = 0 AND pinned = 1 \
        AND ((pin_order IS NOT NULL AND (?1 IS NULL OR pin_order > ?1)) \
             OR (pin_order IS ?1 AND created_at < ?2) \
             OR (pin_order IS ?1 AND created_at = ?2 AND id < ?3)) \
      ORDER BY pin_order ASC, created_at DESC, id DESC \
      LIMIT ?4"
);

/// `created_at <= ?1` is the sargable half; the disjunction that follows only
/// resolves ties inside that one millisecond. `idx_items_evictable`'s partial
/// predicate is this WHERE clause, so the seek starts at the cursor instead of
/// at the newest row in the history.
const UNPINNED_SEEK_SQL: &str = concat!(
    "SELECT ",
    item_columns!(),
    ", pin_order FROM clipboard_items INDEXED BY idx_items_evictable \
      WHERE deleted = 0 AND pinned = 0 \
        AND created_at <= ?1 AND (created_at < ?1 OR id < ?2) \
      ORDER BY created_at DESC, id DESC \
      LIMIT ?3"
);

const UNPINNED_HEAD_SQL: &str = concat!(
    "SELECT ",
    item_columns!(),
    ", pin_order FROM clipboard_items INDEXED BY idx_items_evictable \
      WHERE deleted = 0 AND pinned = 0 \
      ORDER BY created_at DESC, id DESC \
      LIMIT ?1"
);

impl Store {
    /// The page that follows `after`, or the first page when it is `None`.
    ///
    /// Same order and same filter as [`Store::list`] for every row either
    /// writer in `pinning` produces; only the window differs. A short page ends
    /// the list, so `next` is `None` exactly when the caller should stop asking.
    pub fn list_from(&self, after: Option<&ItemCursor>, limit: u32) -> Result<Page, StoreError> {
        let conn = self.conn()?;
        let mut rows: Vec<Row> = Vec::with_capacity(limit as usize);

        // The list starts at the head of the pinned run, so "no cursor" and "a
        // cursor inside the pinned run" are the same case with a different
        // starting point — and both may spill into the unpinned run below.
        let in_pinned_run = after.is_none_or(|cursor| cursor.pinned);
        match after {
            None => fetch(&conn, PINNED_HEAD_SQL, params![i64::from(limit)], &mut rows)?,
            Some(cursor) if cursor.pinned => {
                fetch(
                    &conn,
                    PINNED_SEEK_SQL,
                    params![
                        cursor.pin_order,
                        cursor.created_at,
                        cursor.id.as_str(),
                        i64::from(limit),
                    ],
                    &mut rows,
                )?;
            }
            Some(cursor) => fetch(
                &conn,
                UNPINNED_SEEK_SQL,
                params![cursor.created_at, cursor.id.as_str(), i64::from(limit)],
                &mut rows,
            )?,
        }

        // The unpinned run begins where the pinned one ends, so a page that
        // started among the pins and did not fill continues from its head.
        let short = limit as usize - rows.len().min(limit as usize);
        if short > 0 && in_pinned_run {
            fetch(
                &conn,
                UNPINNED_HEAD_SQL,
                params![i64::try_from(short).unwrap_or(i64::MAX)],
                &mut rows,
            )?;
        }

        let next = (rows.len() as u32 == limit && limit > 0)
            .then(|| rows.last())
            .flatten()
            .map(|(item, pin_order)| ItemCursor {
                pinned: item.pinned,
                pin_order: *pin_order,
                created_at: item.created_at,
                id: item.id.clone(),
            });

        Ok(Page {
            items: rows.into_iter().map(|(item, _)| item).collect(),
            next,
        })
    }

    /// The page after `after`, bounded by both `limit` and ciphertext bytes.
    ///
    /// The first row may exceed `budget` so a caller can always advance past a
    /// legacy oversized item. Later rows stop before the byte budget, and the
    /// returned cursor names the last retained row from this same SQL read.
    pub fn list_from_bounded(
        &self,
        after: Option<&ItemCursor>,
        limit: u32,
        budget: usize,
    ) -> Result<Page, StoreError> {
        if limit == 0 {
            return Ok(Page {
                items: Vec::new(),
                next: None,
            });
        }

        let conn = self.conn()?;
        let limit = limit as usize;
        let mut rows = Vec::new();
        let mut bytes = 0;
        let in_pinned_run = after.is_none_or(|cursor| cursor.pinned);
        let result = match after {
            None => fetch_bounded(
                &conn,
                PINNED_HEAD_SQL,
                params![i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX)],
                limit,
                budget,
                &mut bytes,
                &mut rows,
            )?,
            Some(cursor) if cursor.pinned => fetch_bounded(
                &conn,
                PINNED_SEEK_SQL,
                params![
                    cursor.pin_order,
                    cursor.created_at,
                    cursor.id.as_str(),
                    i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX),
                ],
                limit,
                budget,
                &mut bytes,
                &mut rows,
            )?,
            Some(cursor) => fetch_bounded(
                &conn,
                UNPINNED_SEEK_SQL,
                params![
                    cursor.created_at,
                    cursor.id.as_str(),
                    i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX),
                ],
                limit,
                budget,
                &mut bytes,
                &mut rows,
            )?,
        };

        let result = if result == BoundedFetch::Exhausted && in_pinned_run {
            let remaining = limit - rows.len();
            fetch_bounded(
                &conn,
                UNPINNED_HEAD_SQL,
                params![i64::try_from(remaining.saturating_add(1)).unwrap_or(i64::MAX)],
                limit,
                budget,
                &mut bytes,
                &mut rows,
            )?
        } else {
            result
        };

        let next = (!rows.is_empty()
            && matches!(
                result,
                BoundedFetch::BudgetReached | BoundedFetch::CountReached
            ))
        .then(|| rows.last())
        .flatten()
        .map(cursor_of);
        Ok(Page {
            items: rows.into_iter().map(|(item, _)| item).collect(),
            next,
        })
    }

    /// The sort key of one live row, for a client resuming from an item it
    /// already holds.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotFound`] if the row is gone or is a tombstone — a
    /// tombstone has had its `pinned` and `pin_order` cleared, so its position
    /// in the order no longer exists and seeking from it would skip rows.
    pub fn cursor_for(&self, id: &str) -> Result<ItemCursor, StoreError> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT pinned, pin_order, created_at, id FROM clipboard_items \
              WHERE id = ?1 AND deleted = 0",
            [id],
            |row| {
                Ok(ItemCursor {
                    pinned: row.get("pinned")?,
                    pin_order: row.get("pin_order")?,
                    created_at: row.get("created_at")?,
                    id: row.get("id")?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, plan_of, store, T0};

    /// Walk the whole list one page at a time and assert it equals the list read
    /// in one go: no repeats, no gaps, and it terminates.
    fn walk(store: &Store, page_size: u32) -> Vec<String> {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = store.list_from(cursor.as_ref(), page_size).unwrap();
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
            assert!(seen.len() < 1_000, "pagination did not terminate");
        }
        seen
    }

    #[test]
    fn paging_visits_every_item_exactly_once_in_list_order() {
        let s = store();
        for n in 0..7 {
            s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                .unwrap();
        }
        let expected: Vec<String> = s.list(100, 0).unwrap().into_iter().map(|i| i.id).collect();
        for page_size in [1, 2, 3, 7, 100] {
            assert_eq!(walk(&s, page_size), expected, "page size {page_size}");
        }
    }

    #[test]
    fn paging_crosses_the_pinned_boundary_in_both_directions() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..6 {
            ids.push(
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        // Two pins, so a page boundary can fall inside the pinned section, on
        // the boundary itself, and inside the unpinned section.
        s.set_pinned(&ids[1], true).unwrap();
        s.set_pinned(&ids[4], true).unwrap();

        let expected: Vec<String> = s.list(100, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(expected[0], ids[1], "pinned first, in pin order");
        assert_eq!(expected[1], ids[4]);
        for page_size in [1, 2, 3, 5] {
            assert_eq!(walk(&s, page_size), expected, "page size {page_size}");
        }
    }

    /// `CopyPaste-8ebg.57`, the whole reason this module exists: a capture
    /// landing above the window between two pages must not make the second page
    /// repeat or skip a row. The offset call is shown failing next to it so the
    /// difference is not theoretical.
    #[test]
    fn a_row_inserted_above_the_window_does_not_disturb_the_next_page() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..6 {
            ids.push(
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        // Newest first: item-5, item-4, item-3, item-2, item-1, item-0.
        let first = s.list_from(None, 2).unwrap();
        assert_eq!(
            first.items.iter().map(|i| &i.id).collect::<Vec<_>>(),
            vec![&ids[5], &ids[4]]
        );

        s.insert(item("arrived mid-scroll", T0 + 600_000)).unwrap();

        let second = s.list_from(first.next.as_ref(), 2).unwrap();
        assert_eq!(
            second.items.iter().map(|i| &i.id).collect::<Vec<_>>(),
            vec![&ids[3], &ids[2]],
            "the seek page must continue where it stopped"
        );

        let offset_page: Vec<String> = s.list(2, 2).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(
            offset_page,
            vec![ids[4].clone(), ids[3].clone()],
            "offset repeats item-4, which is the bug"
        );
    }

    #[test]
    fn a_deleted_anchor_row_does_not_strand_the_reader() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..4 {
            ids.push(
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        let first = s.list_from(None, 2).unwrap();
        // The cursor holds the anchor's sort key by value, so deleting the row
        // it names cannot invalidate it.
        s.delete(&first.items[1].id).unwrap();

        let second = s.list_from(first.next.as_ref(), 2).unwrap();
        assert_eq!(
            second.items.iter().map(|i| &i.id).collect::<Vec<_>>(),
            vec![&ids[1], &ids[0]]
        );
    }

    #[test]
    fn a_token_round_trips_and_a_bad_one_is_refused() {
        let s = store();
        s.insert(item("only", T0)).unwrap();
        let page = s.list_from(None, 1).unwrap();
        let cursor = page.next.expect("a full page has a cursor");

        let token = cursor.token();
        assert_eq!(ItemCursor::parse(&token).unwrap(), cursor);
        // Opaque: the id must not be readable straight out of the token.
        assert!(!token.contains(&s.list(1, 0).unwrap()[0].id));

        for bad in ["", "nothex!", "abcdef", &token[..token.len() - 2]] {
            assert!(
                matches!(ItemCursor::parse(bad), Err(StoreError::InvalidCursor)),
                "must refuse: {bad}"
            );
        }
    }

    #[test]
    fn cursor_for_names_a_live_row_and_refuses_a_tombstone() {
        let s = store();
        let a = s.insert(item("first", T0)).unwrap();
        let b = s.insert(item("second", T0 + 60_000)).unwrap();

        let from_b = s.cursor_for(&b.id).unwrap();
        let page = s.list_from(Some(&from_b), 10).unwrap();
        assert_eq!(
            page.items.iter().map(|i| &i.id).collect::<Vec<_>>(),
            vec![&a.id]
        );

        s.delete(&b.id).unwrap();
        assert!(matches!(s.cursor_for(&b.id), Err(StoreError::NotFound)));
        assert!(matches!(s.cursor_for("nope"), Err(StoreError::NotFound)));
    }

    /// The unpinned run is the deep one, and the seek into it has to start at
    /// the cursor. Under the single-predicate form the plan scanned
    /// `idx_items_history` from the newest row on every page, so page N cost the
    /// whole history above it.
    #[test]
    fn the_unpinned_page_seeks_instead_of_scanning_the_history() {
        let s = store();
        s.insert(item("something", T0)).unwrap();

        let plan = plan_of(&s, UNPINNED_SEEK_SQL);
        assert!(
            plan.iter()
                .any(|d| d.contains("SEARCH") && d.contains("idx_items_evictable")),
            "the unpinned page must seek its index, got {plan:?}"
        );
        assert!(
            !plan.iter().any(|d| d.contains("TEMP B-TREE")),
            "the index must supply the order, got {plan:?}"
        );
    }

    /// `pinned = 1` is what bounds the pinned seek to the pinned run, so its
    /// cost follows how many items are pinned and not how deep history is.
    #[test]
    fn the_pinned_page_stays_inside_the_pinned_run() {
        let s = store();
        let id = s.insert(item("something", T0)).unwrap().id;
        s.set_pinned(&id, true).unwrap();

        for (label, sql) in [
            ("pinned head", PINNED_HEAD_SQL),
            ("pinned seek", PINNED_SEEK_SQL),
            ("unpinned head", UNPINNED_HEAD_SQL),
        ] {
            let plan = plan_of(&s, sql);
            assert!(
                !plan.iter().any(|d| d.contains("TEMP B-TREE")),
                "{label} must take its order from an index, got {plan:?}"
            );
        }
        assert!(
            plan_of(&s, PINNED_SEEK_SQL)
                .iter()
                .any(|d| d.contains("idx_items_history")),
            "the pinned seek must use the history index"
        );
    }

    /// Paging stays correct at a depth where a per-page rescan of everything
    /// above the window would be the dominant cost.
    #[test]
    fn a_deep_history_pages_to_the_end_without_repeating_or_skipping() {
        const ROWS: usize = 10_000;
        let s = store();
        for n in 0..ROWS {
            s.insert(item(&format!("item-{n}"), T0 + n as i64 * 60_000))
                .unwrap();
        }

        let mut seen = Vec::with_capacity(ROWS);
        let mut cursor = None;
        loop {
            let page = s.list_from(cursor.as_ref(), 100).unwrap();
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(seen.len(), ROWS);

        let unique: std::collections::HashSet<&String> = seen.iter().collect();
        assert_eq!(unique.len(), ROWS, "a row was repeated across pages");

        let expected: Vec<String> = s
            .list(ROWS as u32, 0)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(seen, expected);
    }

    /// An unpinned row is supposed to have no `pin_order`, and both writers in
    /// `pinning` keep it that way — but a merge writes both columns from a peer.
    ///
    /// This passes on the single-predicate form too, and is here because the
    /// obvious way to make the unpinned seek sargable — adding `pin_order IS
    /// NULL` beside `pinned = 0` — silently drops such a row from the list. The
    /// property to hold is that every live row is still reachable exactly once
    /// at every page size; where it sorts is undefined.
    #[test]
    fn an_unpinned_row_with_a_stray_pin_order_is_listed_once() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..6 {
            ids.push(
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        s.conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET pin_order = 5.0 WHERE id = ?1",
                [&ids[2]],
            )
            .unwrap();

        for page_size in [1, 2, 3, 5] {
            let seen = walk(&s, page_size);
            assert_eq!(seen.len(), 6, "page size {page_size} lost or gained a row");
            let unique: std::collections::HashSet<&String> = seen.iter().collect();
            assert_eq!(unique.len(), 6, "page size {page_size} repeated a row");
            assert!(unique.contains(&ids[2]), "the stray row went missing");
        }
    }

    #[test]
    fn an_empty_store_and_a_zero_limit_both_end_immediately() {
        let s = store();
        let page = s.list_from(None, 10).unwrap();
        assert!(page.items.is_empty());
        assert!(page.next.is_none());

        s.insert(item("something", T0)).unwrap();
        let page = s.list_from(None, 0).unwrap();
        assert!(page.items.is_empty());
        assert!(page.next.is_none(), "a zero limit must not loop forever");
    }

    fn insert_with_ciphertext(store: &Store, label: &str, created_at: i64, bytes: usize) -> String {
        let mut row = item(label, created_at);
        row.content_ciphertext = vec![0; bytes];
        store.insert(row).unwrap().id
    }

    fn make_row_unreadable(store: &Store, id: &str) {
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET content_type = CAST(X'80' AS TEXT) WHERE id = ?1",
                [id],
            )
            .unwrap();
    }

    fn walk_bounded(store: &Store, limit: u32, budget: usize) -> Vec<String> {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = store
                .list_from_bounded(cursor.as_ref(), limit, budget)
                .unwrap();
            seen.extend(page.items.into_iter().map(|row| row.id));
            match page.next {
                Some(next) => cursor = Some(next),
                None => return seen,
            }
            assert!(seen.len() < 1_000, "bounded pagination did not terminate");
        }
    }

    #[test]
    fn bounded_paging_stops_before_rows_past_the_byte_budget_are_materialized() {
        let s = store();
        let malformed = insert_with_ciphertext(&s, "malformed", T0, 1);
        insert_with_ciphertext(&s, "lookahead", T0 + 1, 1);
        let pinned = insert_with_ciphertext(&s, "pinned", T0 + 2, 10);
        s.set_pinned(&pinned, true).unwrap();
        make_row_unreadable(&s, &malformed);

        let first = s.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            first.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&pinned],
            "the first over-budget unpinned row is lookahead only"
        );
        assert!(first.next.is_some(), "the retained pinned row must resume");
    }

    #[test]
    fn bounded_paging_keeps_an_oversize_first_row_and_visits_its_follower_once() {
        let s = store();
        let follower = insert_with_ciphertext(&s, "follower", T0, 1);
        let oversize = insert_with_ciphertext(&s, "oversize", T0 + 1, 11);

        let first = s.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            first.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&oversize]
        );
        let second = s.list_from_bounded(first.next.as_ref(), 1000, 10).unwrap();
        assert_eq!(
            second.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&follower]
        );
    }

    #[test]
    fn bounded_paging_omits_the_cursor_at_an_exact_final_count() {
        for count in [1, 3] {
            let s = store();
            for n in 0..count {
                insert_with_ciphertext(&s, &format!("row-{n}"), T0 + n as i64, 1);
            }

            let page = s.list_from_bounded(None, count as u32, 10).unwrap();
            assert_eq!(page.items.len(), count);
            assert!(
                page.next.is_none(),
                "an exact final page of {count} rows must not invite a retry"
            );
        }
    }

    #[test]
    fn bounded_paging_omits_the_cursor_for_an_exact_pins_only_page() {
        let s = store();
        let first = insert_with_ciphertext(&s, "first", T0, 1);
        let second = insert_with_ciphertext(&s, "second", T0 + 1, 1);
        s.set_pinned(&first, true).unwrap();
        s.set_pinned(&second, true).unwrap();

        let page = s.list_from_bounded(None, 2, 10).unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(
            page.next.is_none(),
            "the final pinned row must end the page"
        );
    }

    #[test]
    fn bounded_paging_checks_unpinned_after_an_exact_pinned_run() {
        let s = store();
        let first = insert_with_ciphertext(&s, "first", T0, 1);
        let second = insert_with_ciphertext(&s, "second", T0 + 1, 1);
        let follower = insert_with_ciphertext(&s, "follower", T0 + 2, 1);
        s.set_pinned(&first, true).unwrap();
        s.set_pinned(&second, true).unwrap();
        let expected: Vec<String> = s
            .list(1000, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();

        let page = s.list_from_bounded(None, 2, 10).unwrap();
        assert_eq!(
            page.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            expected[..2].iter().collect::<Vec<_>>()
        );
        let cursor = page.next.expect("the unpinned follower requires a cursor");
        assert_eq!(
            s.list_from_bounded(Some(&cursor), 2, 10)
                .unwrap()
                .items
                .iter()
                .map(|row| &row.id)
                .collect::<Vec<_>>(),
            vec![&follower]
        );
    }

    #[test]
    fn bounded_paging_ends_after_an_exact_pinned_to_unpinned_page() {
        let s = store();
        let pinned = insert_with_ciphertext(&s, "pinned", T0, 1);
        let unpinned = insert_with_ciphertext(&s, "unpinned", T0 + 1, 1);
        s.set_pinned(&pinned, true).unwrap();

        let page = s.list_from_bounded(None, 2, 10).unwrap();
        assert_eq!(
            page.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&pinned, &unpinned]
        );
        assert!(page.next.is_none());
    }

    #[test]
    fn bounded_paging_sets_a_cursor_after_a_byte_stop_or_oversize_only_with_a_follower() {
        for first_bytes in [6, 11] {
            let final_row = store();
            insert_with_ciphertext(&final_row, "first", T0, first_bytes);
            assert!(final_row
                .list_from_bounded(None, 1000, 10)
                .unwrap()
                .next
                .is_none());

            let followed = store();
            let follower = insert_with_ciphertext(&followed, "follower", T0, 5);
            let first = insert_with_ciphertext(&followed, "first", T0 + 1, first_bytes);
            let page = followed.list_from_bounded(None, 1000, 10).unwrap();
            assert_eq!(
                page.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
                vec![&first]
            );
            let cursor = page.next.expect("a following row must keep the cursor");
            assert_eq!(
                followed
                    .list_from_bounded(Some(&cursor), 1000, 10)
                    .unwrap()
                    .items
                    .iter()
                    .map(|row| &row.id)
                    .collect::<Vec<_>>(),
                vec![&follower]
            );
        }
    }

    #[test]
    fn bounded_paging_reports_a_malformed_count_lookahead_but_not_later_rows() {
        let malformed_lookahead = store();
        let malformed = insert_with_ciphertext(&malformed_lookahead, "malformed", T0, 1);
        insert_with_ciphertext(&malformed_lookahead, "admitted", T0 + 1, 1);
        insert_with_ciphertext(&malformed_lookahead, "later", T0 - 1, 1);
        make_row_unreadable(&malformed_lookahead, &malformed);
        assert!(matches!(
            malformed_lookahead.list_from_bounded(None, 1, 10),
            Err(StoreError::Sqlite(_))
        ));

        let later_malformed = store();
        let later = insert_with_ciphertext(&later_malformed, "later", T0, 1);
        insert_with_ciphertext(&later_malformed, "lookahead", T0 + 1, 1);
        insert_with_ciphertext(&later_malformed, "admitted", T0 + 2, 1);
        make_row_unreadable(&later_malformed, &later);
        assert!(
            later_malformed
                .list_from_bounded(None, 1, 10)
                .unwrap()
                .next
                .is_some(),
            "only one count lookahead may be decoded"
        );
    }

    #[test]
    fn bounded_paging_distinguishes_an_exact_budget_from_one_byte_over() {
        let exact = store();
        let exact_older = insert_with_ciphertext(&exact, "exact older", T0, 5);
        let exact_newer = insert_with_ciphertext(&exact, "exact newer", T0 + 1, 5);
        let exact_later = insert_with_ciphertext(&exact, "exact later", T0 - 1, 1);
        let page = exact.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            page.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&exact_newer, &exact_older]
        );
        assert_eq!(
            exact
                .list_from_bounded(page.next.as_ref(), 1000, 10)
                .unwrap()
                .items
                .iter()
                .map(|row| &row.id)
                .collect::<Vec<_>>(),
            vec![&exact_later]
        );

        let over = store();
        let over_older = insert_with_ciphertext(&over, "over older", T0, 5);
        let over_newer = insert_with_ciphertext(&over, "over newer", T0 + 1, 6);
        let page = over.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            page.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&over_newer]
        );
        assert_eq!(
            over.list_from_bounded(page.next.as_ref(), 1000, 10)
                .unwrap()
                .items
                .iter()
                .map(|row| &row.id)
                .collect::<Vec<_>>(),
            vec![&over_older]
        );
    }

    #[test]
    fn bounded_paging_reports_malformed_first_or_admitted_rows() {
        for malformed_after_admitted in [false, true] {
            let s = store();
            let malformed = insert_with_ciphertext(&s, "malformed", T0, 1);
            if malformed_after_admitted {
                insert_with_ciphertext(&s, "admitted", T0 + 1, 1);
            }
            make_row_unreadable(&s, &malformed);
            assert!(matches!(
                s.list_from_bounded(None, 1000, 10),
                Err(StoreError::Sqlite(_))
            ));
        }
    }

    #[test]
    fn bounded_paging_refuses_a_malformed_over_budget_lookahead_without_partial_page() {
        let s = store();
        let following = insert_with_ciphertext(&s, "following", T0, 1);
        let malformed = insert_with_ciphertext(&s, "malformed", T0 + 1, 1);
        let retained = insert_with_ciphertext(&s, "retained", T0 + 2, 10);
        make_row_unreadable(&s, &malformed);

        assert!(matches!(
            s.list_from_bounded(None, 1000, 10),
            Err(StoreError::Sqlite(_))
        ));

        s.conn()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET content_type = 'text' WHERE id = ?1",
                [&malformed],
            )
            .unwrap();
        let first = s.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            first.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&retained]
        );
        assert_eq!(
            s.list_from_bounded(first.next.as_ref(), 1000, 10)
                .unwrap()
                .items
                .iter()
                .map(|row| &row.id)
                .collect::<Vec<_>>(),
            vec![&malformed, &following]
        );
    }

    #[test]
    fn bounded_paging_walks_pinned_and_unpinned_runs_without_topup_after_a_pin_stop() {
        let fitting = store();
        let fitting_unpinned_old = insert_with_ciphertext(&fitting, "unpinned old", T0, 2);
        let fitting_unpinned_new = insert_with_ciphertext(&fitting, "unpinned new", T0 + 1, 2);
        let fitting_pin_first = insert_with_ciphertext(&fitting, "pin first", T0 + 2, 3);
        let fitting_pin_second = insert_with_ciphertext(&fitting, "pin second", T0 + 3, 3);
        fitting.set_pinned(&fitting_pin_first, true).unwrap();
        fitting.set_pinned(&fitting_pin_second, true).unwrap();
        let expected: Vec<String> = fitting
            .list(1000, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        let first = fitting.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            first.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>(),
            "exhausted pins must fill the remaining byte budget from unpinned rows"
        );
        assert!(first.next.is_none());
        assert_eq!(
            walk_bounded(&fitting, 1000, 10),
            expected,
            "the fitting pinned-to-unpinned page must preserve list order"
        );
        assert_eq!(expected.len(), 4);
        assert!(expected.contains(&fitting_unpinned_old));
        assert!(expected.contains(&fitting_unpinned_new));

        let stopped = store();
        insert_with_ciphertext(&stopped, "unpinned old", T0, 2);
        insert_with_ciphertext(&stopped, "unpinned new", T0 + 1, 2);
        let stopped_pin_first = insert_with_ciphertext(&stopped, "pin first", T0 + 2, 6);
        let stopped_pin_second = insert_with_ciphertext(&stopped, "pin second", T0 + 3, 6);
        stopped.set_pinned(&stopped_pin_first, true).unwrap();
        stopped.set_pinned(&stopped_pin_second, true).unwrap();
        let expected: Vec<String> = stopped
            .list(1000, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();

        let first = stopped.list_from_bounded(None, 1000, 10).unwrap();
        assert_eq!(
            first.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&expected[0]],
            "a byte stop inside pins must not top up from unpinned rows"
        );
        assert_eq!(
            walk_bounded(&stopped, 1000, 10),
            expected,
            "resuming after the pin stop must visit every row once in list order"
        );
    }

    #[test]
    fn bounded_paging_honors_a_thousand_row_limit_and_zero_stops() {
        let s = store();
        for n in 0..1001 {
            insert_with_ciphertext(&s, &format!("item-{n}"), T0 + n, 1);
        }
        let page = s.list_from_bounded(None, 1000, usize::MAX).unwrap();
        assert_eq!(page.items.len(), 1000);
        assert!(page.next.is_some());

        let empty = s.list_from_bounded(None, 0, 10).unwrap();
        assert!(empty.items.is_empty());
        assert!(empty.next.is_none());
    }

    #[test]
    fn deleting_a_bounded_page_anchor_still_allows_the_cursor_to_resume() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..4 {
            ids.push(insert_with_ciphertext(&s, &format!("item-{n}"), T0 + n, 1));
        }
        let first = s.list_from_bounded(None, 2, 10).unwrap();
        s.delete(&first.items[1].id).unwrap();

        let second = s.list_from_bounded(first.next.as_ref(), 2, 10).unwrap();
        assert_eq!(
            second.items.iter().map(|row| &row.id).collect::<Vec<_>>(),
            vec![&ids[1], &ids[0]]
        );
    }
}
