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
//! # Why the branches, and not a row-value comparison
//!
//! SQLite can compare `(a, b, c) < (?, ?, ?)` in one go, but a row-value
//! comparison assumes every column runs in the same direction, and this order
//! mixes DESC and ASC. The expansion below is the portable construction, with
//! `>` on the ASC key and `<` on the DESC ones.
//!
//! `pin_order` is compared with SQLite's null-safe `IS` because it is NULL on
//! every unpinned row, and `=` against NULL is NULL, which would silently drop
//! the branch and end pagination one page early.

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

impl Store {
    /// The page that follows `after`, or the first page when it is `None`.
    ///
    /// Same order and same filter as [`Store::list`]; only the window differs.
    /// A short page ends the list, so `next` is `None` exactly when the caller
    /// should stop asking.
    pub fn list_from(&self, after: Option<&ItemCursor>, limit: u32) -> Result<Page, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            ", pin_order FROM clipboard_items \
              WHERE deleted = 0 \
                AND (?1 = 0 \
                     OR pinned < ?2 \
                     OR (pinned = ?2 AND pin_order IS NOT NULL \
                         AND (?3 IS NULL OR pin_order > ?3)) \
                     OR (pinned = ?2 AND pin_order IS ?3 AND created_at < ?4) \
                     OR (pinned = ?2 AND pin_order IS ?3 AND created_at = ?4 AND id < ?5)) \
              ORDER BY pinned DESC, pin_order ASC, created_at DESC, id DESC \
              LIMIT ?6"
        ))?;

        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map(
            params![
                i64::from(after.is_some()),
                after.is_some_and(|c| c.pinned),
                after.and_then(|c| c.pin_order),
                after.map_or(0, |c| c.created_at),
                after.map_or("", |c| c.id.as_str()),
                i64::from(limit),
            ],
            |row| {
                Ok((
                    row_to_item(row, &columns)?,
                    row.get::<_, Option<f64>>(columns.pin_order_index())?,
                ))
            },
        )?;
        let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;

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
    use crate::storage::test_support::{item, store, T0};

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
}
