//! `pull_items`: keyset-paginated read of a device's sync inbox.

use crate::error::RelayError;
use crate::models::PullItem;

use super::super::MAX_PULL_BYTES_BUDGET;

/// A page returned by [`RelayStore::pull_items`], plus whether more qualifying
/// items exist beyond what was returned.
///
/// `has_more` is `true` when either the byte-budget cap ([`MAX_PULL_BYTES_BUDGET`])
/// broke the collection loop before `limit` items were gathered, or the inbox
/// held more than `limit` qualifying items past the cursor (or both) — see
/// [`RelayStore::pull_items`] (CopyPaste-8ebg.58).
#[derive(Debug, Clone)]
pub struct PullPage {
    pub items: Vec<PullItem>,
    pub has_more: bool,
}

impl super::super::RelayStore {
    /// Return up to `limit` items in `device_id`'s sync inbox strictly after the
    /// `(since, since_id)` composite cursor, ordered ascending.
    ///
    /// # Contract
    ///
    /// This method returns [`RelayError::DeviceNotFound`] for an unknown
    /// `device_id`. In production every call-site goes through
    /// [`Self::verify_token`] first, which already collapses missing-device to
    /// [`RelayError::Unauthorized`]. Callers that skip `verify_token` will
    /// observe a `DeviceNotFound` rather than `Unauthorized` — that is
    /// intentional: `pull_items` is a pure data accessor with no security
    /// semantics of its own. **Always call `verify_token` before `pull_items`**
    /// on any authenticated route.
    ///
    /// Pagination is driven by a strictly-monotonic `(wall_time, id)` tuple
    /// rather than bare `wall_time` (relay H-1 / audit finding G). `wall_time`
    /// is a sender-supplied millisecond timestamp, so ties are possible; a
    /// `wall_time`-only cursor with a strict `>` floor would skip every item
    /// sharing a boundary timestamp when a page boundary fell mid-run, silently
    /// dropping items. The per-device `id` is unique and ascending, so the tuple
    /// `(wall_time, id)` is a total order with no ties: items qualify iff
    /// `(item.wall_time, item.id) > (since, since_id)`.
    ///
    /// `since_id` is optional for backward compatibility: when `None` the cursor
    /// degrades to the historical `wall_time`-only floor (`wall_time > since`),
    /// matching pre-cursor clients. New clients paginate by feeding back the
    /// last returned `(wall_time, id)` as `(since, since_id)`.
    ///
    /// The inbox is kept sorted by `wall_time` on insert (see `push_item`),
    /// and within an equal `wall_time` run `id` is ascending too (ids are issued
    /// monotonically and ties preserve insertion order), so the inbox is sorted
    /// by the full `(wall_time, id)` tuple. This binary-searches for the first
    /// item past the cursor and clones only the (at most `limit`) items it
    /// returns — it never clones+sorts the whole inbox under the global mutex
    /// (M4). A `limit` of `0` is treated as "no items" rather than "unbounded";
    /// callers wanting the whole window pass a large explicit cap.
    ///
    /// # `has_more` (CopyPaste-8ebg.58)
    ///
    /// A short return (fewer than `limit` items) is ambiguous on its own: it
    /// can mean either "the inbox is exhausted" or "the
    /// [`MAX_PULL_BYTES_BUDGET`] byte cap was hit mid-page" (see the `break`
    /// below). [`PullPage::has_more`] disambiguates this explicitly: it is
    /// `true` when the byte-budget broke the loop before `limit` items were
    /// collected, OR when there were more than `limit` qualifying items past
    /// the cursor to begin with (or both) — so callers no longer need to infer
    /// "caught up" from `items.len() < limit`.
    pub fn pull_items(
        &self,
        device_id: &str,
        since: u64,
        since_id: Option<i64>,
        limit: usize,
    ) -> Result<PullPage, RelayError> {
        let inbox = self
            .sync_items
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        // First index strictly past the cursor. The inbox is sorted ascending by
        // `(wall_time, id)`, so everything from `start` onward qualifies (no full
        // scan/sort). With `since_id` we advance past every item up to and
        // including the cursor tuple; without it we fall back to the legacy
        // `wall_time`-only floor (`wall_time <= since`).
        let start = match since_id {
            Some(since_id) => {
                inbox.partition_point(|item| (item.wall_time, item.id) <= (since, since_id))
            }
            None => inbox.partition_point(|item| item.wall_time <= since),
        };

        // Collect at most `limit` items but also enforce a byte-budget cap
        // (MAX_PULL_BYTES_BUDGET) on the total content_b64 bytes cloned under
        // the global mutex. Without this an authenticated caller with
        // limit=MAX_PULL_LIMIT items × up to 10 MiB each could force ~5 GiB
        // of cloning while holding the lock, stalling all other requests (DoS).
        let mut budget_remaining = MAX_PULL_BYTES_BUDGET;
        let mut result = Vec::new();
        // Truncation cause (a): the byte budget broke the loop before `limit`
        // items (or the whole qualifying window) were collected.
        let mut budget_truncated = false;
        let qualifying = &inbox[start..];
        for item in qualifying.iter().take(limit) {
            let item_bytes = item.content_b64.len();
            if item_bytes > budget_remaining {
                budget_truncated = true;
                break;
            }
            budget_remaining -= item_bytes;
            result.push(PullItem {
                id: item.id,
                content_type: item.content_type.clone(),
                // CopyPaste-ux2i: refcount bump, not a full-payload memcpy.
                content_b64: std::sync::Arc::clone(&item.content_b64),
                wall_time: item.wall_time,
            });
        }

        // Truncation cause (b): there were more qualifying items than `limit`
        // itself, independent of the byte budget.
        let limit_truncated = qualifying.len() > limit;
        let has_more = budget_truncated || limit_truncated;

        Ok(PullPage {
            items: result,
            has_more,
        })
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::error::RelayError;
    use crate::state::test_helpers::*;
    use crate::state::MAX_PULL_BYTES_BUDGET;

    #[test]
    fn pull_returns_items_since_wall_time() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        push_text(&mut store, &device_a_id(), 3000);
        let items = store
            .pull_items(&device_a_id(), 1000, None, usize::MAX)
            .unwrap()
            .items;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].wall_time, 2000);
        assert_eq!(items[1].wall_time, 3000);
    }

    #[test]
    fn pull_since_zero_returns_all() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 100);
        push_text(&mut store, &device_a_id(), 200);
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap()
            .items;
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn pull_sorted_ascending_by_wall_time() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 3000);
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap()
            .items;
        let times: Vec<u64> = items.iter().map(|i| i.wall_time).collect();
        assert_eq!(times, vec![1000, 2000, 3000]);
    }

    #[test]
    fn pull_returns_device_not_found_for_unknown_device() {
        let store = make_store();
        let err = store
            .pull_items("unknown-device", 0, None, usize::MAX)
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceNotFound));
    }

    /// `pull_items` must honor `limit`, returning at most `limit` items.
    #[test]
    fn pull_items_respects_limit() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in 1u64..=10 {
            push_text(&mut store, &device_a_id(), t);
        }
        let page = store.pull_items(&device_a_id(), 0, None, 3).unwrap().items;
        assert_eq!(page.len(), 3, "limit must cap the page size");
        assert_eq!(
            page.iter().map(|i| i.wall_time).collect::<Vec<_>>(),
            vec![1, 2, 3],
        );
    }

    /// Pagination via `since` + `limit` must walk the whole window without gaps
    /// or duplicates.
    #[test]
    fn pull_items_pagination_walks_window() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in 1u64..=5 {
            push_text(&mut store, &device_a_id(), t);
        }
        let mut seen = Vec::new();
        let mut since = 0u64;
        loop {
            let page = store
                .pull_items(&device_a_id(), since, None, 2)
                .unwrap()
                .items;
            if page.is_empty() {
                break;
            }
            since = page.last().unwrap().wall_time;
            seen.extend(page.iter().map(|i| i.wall_time));
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 5]);
    }

    /// Pagination must not drop items when a page boundary falls in the middle
    /// of a run of equal `wall_time` values. The composite `(wall_time, id)`
    /// cursor must walk the whole tied run.
    #[test]
    fn pull_items_pagination_no_drop_on_tied_wall_times() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        let id1 = push_text(&mut store, &device_a_id(), 10);
        let id2 = push_text(&mut store, &device_a_id(), 10);
        let id3 = push_text(&mut store, &device_a_id(), 10);

        let page1 = store.pull_items(&device_a_id(), 0, None, 2).unwrap().items;
        assert_eq!(page1.len(), 2);
        assert_eq!(
            page1.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![id1, id2]
        );

        let last = page1.last().unwrap();
        let page2 = store
            .pull_items(&device_a_id(), last.wall_time, Some(last.id), 2)
            .unwrap()
            .items;
        assert_eq!(
            page2.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![id3],
            "composite cursor must return the remaining tied item"
        );

        // Full walk must see every item exactly once.
        let mut seen_ids = Vec::new();
        let mut since = 0u64;
        let mut since_id: Option<i64> = None;
        loop {
            let page = store
                .pull_items(&device_a_id(), since, since_id, 2)
                .unwrap()
                .items;
            if page.is_empty() {
                break;
            }
            let last = page.last().unwrap();
            since = last.wall_time;
            since_id = Some(last.id);
            seen_ids.extend(page.iter().map(|i| i.id));
        }
        assert_eq!(seen_ids, vec![id1, id2, id3]);
    }

    /// Out-of-order pushes must still be returned ascending by `wall_time`.
    #[test]
    fn pull_items_ordered_after_out_of_order_push() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in [50u64, 10, 30, 20, 40] {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap()
            .items;
        assert_eq!(
            items.iter().map(|i| i.wall_time).collect::<Vec<_>>(),
            vec![10, 20, 30, 40, 50]
        );
    }

    // ---- MAX_PULL_BYTES_BUDGET accessible from this module ------------------

    /// Smoke-test that the byte-budget constant is visible and has the expected
    /// value (128 MiB), guarding against accidental rewrites.
    #[test]
    fn max_pull_bytes_budget_is_128_mib() {
        assert_eq!(MAX_PULL_BYTES_BUDGET, 128 * 1024 * 1024);
    }
}
