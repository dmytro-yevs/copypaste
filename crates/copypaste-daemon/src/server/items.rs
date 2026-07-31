//! The handlers for the history operations, and the decrypt-to-wire step they
//! all share.
//!
//! Everything here is blocking (SQLite, AEAD, the pasteboard) and is reached
//! through one `spawn_blocking` hop in [`super::dispatch`]. The peer operations
//! are network I/O and live in `crate::p2p::handlers` instead — the split
//! between the two files is the split between the two thread pools.

use copypaste_core::{ItemCursor, StoredItem};
use copypaste_ipc::{
    ErrorCode, Item, ItemPage, Response, ResponseData, StatusData, PROTOCOL_VERSION,
};
use tracing::{error, warn};

use super::messages::{
    decrypt_error, storage_error, MSG_BAD_CURSOR, MSG_CLIPBOARD, MSG_EMPTY_CONTENT, MSG_ENCRYPT,
    MSG_NOT_FOUND, MSG_REORDER_TOO_MANY, MSG_TOO_BIG,
};
use crate::capture::{self, IngestError};
use crate::AppState;

/// Server-side clamp on any caller-supplied page size (manifest 04 §3.3,
/// `MAX_PAGE`). A client asking for 10 million rows gets 1 000.
const MAX_PAGE: u32 = 1_000;
/// Applied when `list` is called with `limit = 0`.
const DEFAULT_LIST_PAGE: u32 = 50;
/// Applied when `search` is called with `limit = 0`.
const DEFAULT_SEARCH_PAGE: u32 = 20;
/// Ceiling on one `reorder_pinned` request.
///
/// The frame cap already bounds the bytes; this bounds the *work*, so one
/// request cannot hold the write transaction over a list nobody could have
/// dragged. It is above any plausible pinned section — the history cap itself
/// is 10 000 items — so a real ordering is never refused.
const MAX_REORDER_IDS: usize = 10_000;

pub(super) fn status(state: &AppState, id: u64) -> Response {
    // `status` never fails: an unreadable count is reported as zero rather than
    // turned into an error, because the caller may be probing precisely because
    // the database is unhappy.
    let item_count = match state.store.count() {
        Ok(count) => count,
        Err(e) => {
            warn!(error = ?e, "could not count items for status");
            0
        }
    };

    Response::ok(
        id,
        ResponseData::Status(StatusData {
            version: crate::DAEMON_VERSION.to_string(),
            protocol_version: PROTOCOL_VERSION,
            item_count,
            capture_running: state.capture_running(),
            clipboard_backend: state.backend_name().to_string(),
            legacy_history_present: state.legacy_history_present(),
            counters: state.counters(),
        }),
    )
}

pub(super) fn list(state: &AppState, id: u64, limit: u32, cursor: Option<&str>) -> Response {
    let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
    let after = match cursor.map(ItemCursor::parse).transpose() {
        Ok(after) => after,
        // Never "start from the top": a load-more that silently restarted would
        // repeat the whole history, and the client cannot tell that from a list
        // that really does begin again.
        Err(_) => return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_CURSOR),
    };

    match state.store.list_from(after.as_ref(), limit) {
        Ok(page) => {
            let next = page.next.map(|cursor| cursor.token());
            let mut wire = decrypt_rows(state, page.items);
            wire.next_cursor = next;
            Response::ok(id, ResponseData::Page(wire))
        }
        Err(e) => storage_error(id, "list", &e),
    }
}

pub(super) fn search(state: &AppState, id: u64, query: &str, limit: u32) -> Response {
    let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
    match state.store.search(query, limit) {
        Ok(rows) => {
            // Read-time enforcement of "sensitive items are never searchable".
            // The store already keeps them out of the index at write time; this
            // is the second of the three layers the rule demands, and it is
            // what protects a database written before the rule existed.
            let rows: Vec<StoredItem> = rows.into_iter().filter(|row| !row.is_sensitive).collect();
            Response::ok(id, ResponseData::Page(decrypt_rows(state, rows)))
        }
        Err(e) => storage_error(id, "search", &e),
    }
}

pub(super) fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", &e),
    };

    let item = match to_wire(state, row) {
        Ok(item) => item,
        Err(e) => return decrypt_error(id, &e),
    };

    if let Err(e) = state.clipboard().set_contents(&item.content) {
        error!(error = ?e, "pasteboard write failed");
        return Response::err(id, ErrorCode::Internal, MSG_CLIPBOARD);
    }

    Response::ok(id, ResponseData::Item(item))
}

pub(super) fn add(state: &AppState, id: u64, content: &str) -> Response {
    // Same ingest path as the capture loop: detector, encrypt, dedup, insert,
    // evict. `add` cannot skip the detector — an item entering here is exactly
    // as likely to be a credential as one copied from the pasteboard.
    match capture::ingest(state, content, copypaste_ipc::content_type::TEXT) {
        Ok(ingested) => match to_wire(state, ingested.into_item()) {
            Ok(item) => {
                state.note_local_change();
                Response::ok(id, ResponseData::Item(item))
            }
            Err(e) => decrypt_error(id, &e),
        },
        Err(IngestError::Empty) => Response::err(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT),
        Err(IngestError::TooLarge) => Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_BIG),
        Err(e @ IngestError::Crypto(_)) => {
            error!(error = ?e, "add failed to encrypt");
            Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
        }
        Err(IngestError::Storage(e)) => storage_error(id, "add", &e),
    }
}

/// One item, decrypted, with nothing else touched.
///
/// Deliberately not `copy`: reading an item must not publish it to the system
/// pasteboard as a side effect. The two handlers share `to_wire` and differ in
/// exactly that.
pub(super) fn get(state: &AppState, id: u64, item_id: &str) -> Response {
    match state.store.get(item_id) {
        Ok(Some(row)) => match to_wire(state, row) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, &e),
        },
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => storage_error(id, "get", &e),
    }
}

pub(super) fn delete(state: &AppState, id: u64, item_id: &str) -> Response {
    // Read first so an unknown id is `not_found` rather than a silent success:
    // a client that deleted nothing needs to know it deleted nothing.
    let created_at = match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", &e),
        Ok(Some(row)) => row.created_at,
    };

    match state.store.delete(item_id) {
        Ok(_) => {
            // A tombstone keeps the item's original `created_at`, so a delete
            // of anything older than the cloud upload cursor is invisible to it
            // until the cursor is pulled back.
            crate::cloud::note_version_written(state, created_at);
            state.note_local_change();
            Response::ok(id, ResponseData::Empty {})
        }
        Err(e) => storage_error(id, "delete", &e),
    }
}

pub(super) fn delete_all(state: &AppState, id: u64) -> Response {
    // Manifest 03 (`CopyPaste-cb7u`) has `delete_all` tombstone only the
    // non-pinned rows — a pin is the user saying "keep this". `Store::delete_all`
    // currently clears pinned rows too; that is a storage-layer decision and it
    // is deliberately not second-guessed here, because filtering in the server
    // would put the rule in two places and leave the store's own callers with
    // the other behaviour.
    match state.store.delete_all() {
        Ok(deleted) => {
            if deleted > 0 {
                // Every tombstone keeps its item's original stamp, so the cloud
                // cursor has to be pulled back to the oldest of them for the
                // clear to propagate at all.
                match state.store.oldest_version_ms() {
                    Ok(Some(oldest)) => crate::cloud::note_version_written(state, oldest),
                    Ok(None) => {}
                    Err(e) => warn!(error = ?e, "could not reset the cloud upload cursor"),
                }
                state.note_local_change();
            }
            Response::ok(id, ResponseData::Count(deleted))
        }
        Err(e) => storage_error(id, "delete_all", &e),
    }
}

pub(super) fn pin(state: &AppState, id: u64, item_id: &str, pinned: bool) -> Response {
    match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", &e),
        Ok(Some(_)) => {}
    }

    if let Err(e) = state.store.set_pinned(item_id, pinned) {
        return storage_error(id, "set_pinned", &e);
    }
    state.note_local_change();

    // Reply with the updated row so a client does not have to re-list to learn
    // the new state.
    match state.store.get(item_id) {
        Ok(Some(row)) => match to_wire(state, row) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, &e),
        },
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => storage_error(id, "get", &e),
    }
}

/// Rewrite the pinned ordering.
///
/// The whole ordering, applied in one transaction by `Store::reorder_pinned` —
/// see [`copypaste_ipc::Method::ReorderPinned`] for why a move-one verb is not
/// offered. Ids the caller no longer owns are ignored there rather than
/// refused here, so a peer deleting a pinned item between the client's read and
/// its write does not lose the reorder the user just made.
///
/// Answers with the count of rows renumbered — which may exceed `ids.len()`,
/// because pinned rows the caller did not name are renumbered behind the ones
/// it did.
pub(super) fn reorder_pinned(state: &AppState, id: u64, ids: &[String]) -> Response {
    if ids.len() > MAX_REORDER_IDS {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_REORDER_TOO_MANY);
    }
    match state.store.reorder_pinned(ids) {
        Ok(0) => Response::ok(id, ResponseData::Count(0)),
        Ok(renumbered) => {
            // A pin is local and never travels (`Store::upsert` preserves both
            // `pinned` and `pin_order` across an incoming version), so this
            // wakes the watchers and nothing else. Waking the sync loops for a
            // change no transport will carry would be a round trip per drag.
            state.note_remote_change();
            Response::ok(id, ResponseData::Count(renumbered))
        }
        Err(e) => storage_error(id, "reorder_pinned", &e),
    }
}

/// Decrypt a stored row into its wire form, resolving its origin as it goes.
///
/// One row, one origin lookup. [`decrypt_rows`] resolves a whole page in one
/// query instead — see [`to_wire_with`], which is what the two share.
fn to_wire(state: &AppState, row: StoredItem) -> Result<Item, copypaste_core::CryptoError> {
    let origin = state.meta.origin_of(&row).unwrap_or_else(|e| {
        // Attribution is advisory: a row whose origin cannot be read is still
        // the user's item, and the fallback is the same one the origin table's
        // absence already means — this device captured it.
        warn!(error = ?e, "could not resolve an item's origin device");
        state.meta.here()
    });
    to_wire_with(state, row, &origin)
}

/// [`to_wire`] with the origin already resolved.
fn to_wire_with(
    state: &AppState,
    row: StoredItem,
    origin: &crate::meta::Origin,
) -> Result<Item, copypaste_core::CryptoError> {
    let key = state.keyring.item_key();
    // The item id is the AAD: a row decrypted under another row's identity must
    // fail authentication, not fall back to a plaintext read (CLAUDE.md rule 4,
    // "fail closed on crypto").
    let plaintext = copypaste_core::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)?;
    // Measured on the plaintext bytes, because that is what the cloud path
    // measures: `LocalItem::content` is the opened payload, and the seal that
    // follows is a fixed overhead the cap does not count.
    let too_large_to_sync = crate::cloud::too_large_to_sync(&row.content_type, plaintext.len());
    Ok(Item {
        id: row.id,
        content: String::from_utf8_lossy(&plaintext).into_owned(),
        content_type: row.content_type,
        created_at: row.created_at,
        pinned: row.pinned,
        is_sensitive: row.is_sensitive,
        origin_device_id: origin.device_id.clone(),
        origin_device_name: origin.device_name.clone(),
        too_large_to_sync,
    })
}

/// Decrypt a page of rows, dropping any row that will not open — and saying how
/// many.
///
/// One unreadable row must not blank an entire page of history: the other items
/// are still the user's data. But a page that is silently one item shorter, with
/// the reason only in the daemon's log, is what v1 shipped and what
/// `CopyPaste-00zz` fixed — the user sees fewer items and is told nothing. The
/// count goes back on the wire so a client can say "3 items could not be read".
fn decrypt_rows(state: &AppState, rows: Vec<StoredItem>) -> ItemPage {
    let mut page = ItemPage {
        items: Vec::with_capacity(rows.len()),
        skipped_undecryptable: 0,
        // Set by `list` from the store's own page, never derived from what
        // survived decryption: a page shortened by unreadable rows is still a
        // full page of the list, and ending on it would hide the history behind
        // them.
        next_cursor: None,
    };
    // One query for the page's attribution rather than one per row: a page is
    // up to `MAX_PAGE` items and this runs on every list and every search.
    let origins = state.meta.origins_for(&rows).unwrap_or_else(|e| {
        warn!(error = ?e, "could not resolve the origin devices for a page");
        std::collections::HashMap::new()
    });
    let here = state.meta.here();
    for row in rows {
        let row_id = row.id.clone();
        let origin = origins.get(&row_id).unwrap_or(&here);
        match to_wire_with(state, row, origin) {
            Ok(item) => page.items.push(item),
            Err(e) => {
                warn!(id = %row_id, error = ?e, "skipping an item that failed to decrypt");
                page.skipped_undecryptable += 1;
            }
        }
    }
    page
}

fn clamp_page(limit: u32, default: u32) -> u32 {
    if limit == 0 {
        default
    } else {
        limit.min(MAX_PAGE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::dispatch::dispatch_store;
    use crate::testutil::test_state;
    use copypaste_ipc::{content_type::TEXT, Method};

    /// I-39 / §6.5: the two clipboard counters existed and nothing read them,
    /// which is the same as not having them. This is the caller that made
    /// deleting their `allow(dead_code)` correct.
    #[test]
    fn status_carries_the_counters_nothing_used_to_read() {
        let (state, _dir) = test_state("counters");
        state.note_sensitive_swept(2);
        state.set_index_purged(7);

        let counters = match status(&state, 1).data {
            Some(ResponseData::Status(s)) => s.counters,
            other => panic!("{other:?}"),
        };
        assert_eq!(counters.sensitive_swept, 2);
        assert_eq!(counters.index_purged, 7);
        // Reached the port rather than being defaulted at the wire: the fake
        // backend answers 0 for both, and the point is that it was asked.
        assert_eq!(counters.rejected_too_large, 0);
        assert_eq!(counters.lost_intermediates, 0);
    }

    /// Rule 4. `status` is the one reply a support flow is built on, so a path
    /// reaching it would be pasted into every issue.
    #[test]
    fn status_never_carries_a_path() {
        let (state, _dir) = test_state("status-paths");
        let json = serde_json::to_string(&status(&state, 1)).unwrap();
        assert_eq!(json, copypaste_ipc::redact::scrub_paths(&json), "{json}");
    }

    #[test]
    fn page_sizes_are_clamped() {
        assert_eq!(clamp_page(0, DEFAULT_LIST_PAGE), DEFAULT_LIST_PAGE);
        assert_eq!(clamp_page(10, DEFAULT_LIST_PAGE), 10);
        assert_eq!(clamp_page(u32::MAX, DEFAULT_LIST_PAGE), MAX_PAGE);
    }

    /// A sensitive item is stored, is visible in `list`, and is never returned
    /// by `search`.
    ///
    /// The store keeps it out of the FTS index at write time; this covers the
    /// server's read-time layer, which is what protects a database written
    /// before the rule existed (CLAUDE.md rule 4 — "enforced at write time, at
    /// read time, and by a purge migration").
    /// Reading must never have the side effect of copying: `get` returns the
    /// content, and the clipboard is untouched by it.
    #[test]
    fn get_returns_an_item_without_touching_the_clipboard() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("server");
        let added = match add(&state, 1, "readable").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };

        match get(&state, 2, &added.id).data {
            Some(ResponseData::Item(item)) => assert_eq!(item.content, "readable"),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            writes.count(),
            0,
            "reading an item published it to the pasteboard"
        );

        // ...and `copy` is the one that does write, so the assertion above is
        // not passing for want of a working clipboard.
        assert!(copy(&state, 3, &added.id).ok);
        assert_eq!(writes.count(), 1);

        // An unknown id is not_found, not an empty success.
        let missing = get(&state, 4, "00000000-0000-0000-0000-000000000000");
        assert_eq!(missing.error_code, Some(ErrorCode::NotFound));
    }

    #[test]
    fn search_never_returns_a_sensitive_item() {
        let (state, _dir) = test_state("server");

        let secret = "AKIAIOSFODNN7EXAMPLE";
        let response = dispatch_store(
            &state,
            1,
            Method::Add {
                content: secret.into(),
            },
        );
        let added = match response.data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected an item, got {other:?}"),
        };
        assert!(added.is_sensitive, "the detector must flag an AWS key id");

        // Data loss is the worse outcome: flagged, but still stored and still
        // listed.
        let response = dispatch_store(
            &state,
            2,
            Method::List {
                limit: 50,
                cursor: None,
            },
        );
        match response.data {
            Some(ResponseData::Page(page)) => {
                assert!(page.items.iter().any(|item| item.id == added.id));
            }
            other => panic!("expected a page, got {other:?}"),
        }

        let response = dispatch_store(
            &state,
            3,
            Method::Search {
                query: secret.into(),
                limit: 50,
            },
        );
        match response.data {
            Some(ResponseData::Page(page)) => assert!(
                !page.items.iter().any(|item| item.id == added.id),
                "a sensitive item reached the search results"
            ),
            other => panic!("expected a page, got {other:?}"),
        }
    }

    /// B-1 / `CopyPaste-8ebg.57`, at the wire: a capture landing above the
    /// window between two pages must not make the second page repeat a row or
    /// skip one. A clipboard manager inserts above the window every time the
    /// user copies anything, so this is the ordinary case and not an edge one.
    ///
    /// The offset that page 2 *would* have used is asserted wrong in the same
    /// test, so the difference is demonstrated rather than claimed.
    #[test]
    fn a_capture_between_two_pages_neither_repeats_nor_skips_a_row() {
        let (state, _dir) = test_state("server");
        // Stamped explicitly rather than by the clock: six captures inside one
        // millisecond order by the `id` tiebreak, which is correct but makes
        // "the new row lands above the window" a coin toss the assertion below
        // needs to be certain of.
        let now = copypaste_core::now_ms();
        let original: Vec<String> = (0..6)
            .map(|n| {
                capture::ingest_at(&state, &format!("clip {n}"), TEXT, now - (10 - n) * 60_000)
                    .expect("ingest")
                    .into_item()
                    .id
            })
            .collect();

        let page = |cursor: Option<&str>| match list(&state, 2, 2, cursor).data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };

        let first = page(None);
        let cursor = first.next_cursor.clone().expect("a full page resumes");

        // The capture that lands mid-scroll — the ordinary case for a clipboard
        // manager. Everything the user has not reached must still be reachable.
        capture::ingest_at(&state, "arrived mid-scroll", TEXT, now).expect("ingest");

        let mut seen: Vec<String> = first.items.iter().map(|i| i.id.clone()).collect();
        let mut next = Some(cursor);
        while let Some(cursor) = next {
            let page = page(Some(&cursor));
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            next = page.next_cursor;
        }

        for id in &original {
            assert_eq!(
                seen.iter().filter(|s| *s == id).count(),
                1,
                "an item was skipped or repeated across the pages"
            );
        }

        // What offset pagination would have done instead: page 1 held two rows,
        // so page 2 is `OFFSET 2` — and the new capture has pushed the row that
        // was last on page 1 down into it.
        let offset_page = state.store.list(2, 2).unwrap();
        assert_eq!(
            offset_page[0].id, first.items[1].id,
            "offset would have repeated the last row of page 1"
        );
    }

    /// A token this build did not write is refused, not silently treated as
    /// "start again": a load-more that restarted would replay the whole history
    /// and the client could not tell.
    #[test]
    fn a_forged_cursor_is_refused_rather_than_restarting_the_list() {
        let (state, _dir) = test_state("server");
        assert!(add(&state, 1, "one").ok);

        for bad in ["", "not-hex!", "abcdef"] {
            let response = list(&state, 2, 10, Some(bad));
            assert_eq!(
                response.error_code,
                Some(ErrorCode::InvalidRequest),
                "accepted {bad:?}"
            );
        }
    }

    /// A pin reorders the list under the reader. The cursor carries the sort
    /// key by value, so the anchor moving to the pinned section neither strands
    /// the reader nor duplicates the rows behind it.
    #[test]
    fn pinning_the_anchor_row_does_not_disturb_the_rest_of_the_walk() {
        let (state, _dir) = test_state("server");
        let now = copypaste_core::now_ms();
        let ids: Vec<String> = (0..5)
            .map(|n| {
                capture::ingest_at(&state, &format!("clip {n}"), TEXT, now - (10 - n) * 60_000)
                    .expect("ingest")
                    .into_item()
                    .id
            })
            .collect();

        let first = match list(&state, 2, 2, None).data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };
        let anchor = first.items[1].id.clone();
        assert!(pin(&state, 3, &anchor, true).ok);

        let mut seen: Vec<String> = Vec::new();
        let mut next = first.next_cursor;
        while let Some(cursor) = next {
            let page = match list(&state, 4, 2, Some(&cursor)).data {
                Some(ResponseData::Page(page)) => page,
                other => panic!("{other:?}"),
            };
            seen.extend(page.items.iter().map(|i| i.id.clone()));
            next = page.next_cursor;
        }

        assert!(
            !seen.contains(&anchor),
            "the pinned anchor came back a second time"
        );
        for id in ids
            .iter()
            .filter(|id| **id != anchor && **id != first.items[0].id)
        {
            assert_eq!(
                seen.iter().filter(|s| *s == id).count(),
                1,
                "a row was lost or repeated when the anchor was pinned"
            );
        }
    }

    /// `add` and the capture loop share one ingest path, so the same content
    /// twice inside the dedup window is one row.
    #[test]
    fn adding_the_same_content_twice_deduplicates() {
        let (state, _dir) = test_state("server");

        let add = |id| {
            let response = dispatch_store(
                &state,
                id,
                Method::Add {
                    content: "the same thing".into(),
                },
            );
            match response.data {
                Some(ResponseData::Item(item)) => item,
                other => panic!("expected an item, got {other:?}"),
            }
        };

        let first = add(1);
        let second = add(2);
        assert_eq!(first.id, second.id);
        assert_eq!(state.store.count().unwrap(), 1);
    }

    /// The whole ordering, applied at once. A partial move is not offered
    /// because it is ambiguous once a peer pins something in between.
    #[test]
    fn reordering_rewrites_the_pinned_section() {
        let (state, _dir) = test_state("server");
        let ids: Vec<String> = ["alpha", "bravo", "charlie"]
            .iter()
            .map(|content| {
                let added = match add(&state, 1, content).data {
                    Some(ResponseData::Item(item)) => item,
                    other => panic!("{other:?}"),
                };
                assert!(pin(&state, 2, &added.id, true).ok);
                added.id
            })
            .collect();

        let listed = |state: &AppState| match list(state, 3, 10, None).data {
            Some(ResponseData::Page(page)) => page
                .items
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        };
        assert_eq!(listed(&state), ids, "pins list in the order they were made");

        let reversed: Vec<String> = ids.iter().rev().cloned().collect();
        match reorder_pinned(&state, 4, &reversed).data {
            Some(ResponseData::Count(renumbered)) => assert_eq!(renumbered, 3),
            other => panic!("{other:?}"),
        }
        assert_eq!(listed(&state), reversed);
    }

    /// An id the client no longer owns is ignored rather than refused: the list
    /// it is reordering was read a moment ago, and a sync round may have taken
    /// one since. Failing the whole gesture would lose the drag the user made.
    #[test]
    fn an_unknown_id_does_not_fail_the_reorder() {
        let (state, _dir) = test_state("server");
        let added = match add(&state, 1, "kept").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert!(pin(&state, 2, &added.id, true).ok);

        let response = reorder_pinned(
            &state,
            3,
            &["00000000-0000-4000-8000-000000000000".to_string(), added.id],
        );
        assert!(response.ok, "{:?}", response.error);
    }

    #[test]
    fn an_implausibly_long_reorder_is_refused_before_the_transaction() {
        let (state, _dir) = test_state("server");
        let ids = vec!["x".to_string(); MAX_REORDER_IDS + 1];
        let response = reorder_pinned(&state, 1, &ids);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
    }

    /// UI audit finding 3: with sync on, a row must say which device it came
    /// from. An item captured here reports this device, by name.
    #[test]
    fn a_listed_item_carries_its_origin_device() {
        let (state, _dir) = test_state("server");
        let mine = match add(&state, 1, "captured here").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(mine.origin_device_id, state.meta.device_id());
        assert_eq!(
            mine.origin_device_name.as_deref(),
            Some(state.meta.device_name())
        );

        // A row that arrived from elsewhere must not read as local, and it has
        // no name until a session with that device has told us one. Applied
        // through the real merge path a peer's item takes, because that is what
        // stamps `origin_device_id` on the row.
        crate::sync::store_source(&state)
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "from-the-phone",
                content: "arrived over the peer transport",
                content_type: copypaste_ipc::content_type::TEXT,
                created_at: copypaste_core::now_ms(),
                deleted: false,
                content_hash: None,
                origin_device_id: "device-b",
            })
            .expect("the merge must take an unknown item");

        match get(&state, 2, "from-the-phone").data {
            Some(ResponseData::Item(item)) => {
                assert_eq!(item.origin_device_id, "device-b");
                assert_ne!(item.origin_device_id, state.meta.device_id());
                assert_eq!(item.origin_device_name, None, "no session has named it yet");
            }
            other => panic!("{other:?}"),
        }

        state.meta.record_device_name("device-b", "Phone").unwrap();
        match list(&state, 3, 10, None).data {
            Some(ResponseData::Page(page)) => {
                let theirs = page
                    .items
                    .iter()
                    .find(|item| item.id == "from-the-phone")
                    .expect("listed");
                assert_eq!(theirs.origin_device_name.as_deref(), Some("Phone"));
                let ours = page.items.iter().find(|item| item.id == mine.id).unwrap();
                assert_eq!(
                    ours.origin_device_name.as_deref(),
                    Some(state.meta.device_name())
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// `CopyPaste-f72f` / UI audit finding 9. An ordinary clip is carryable and
    /// must not be flagged; the flag itself is exercised against the cap in
    /// `cloud::tests`, because a real one needs an 8 MiB item.
    #[test]
    fn an_ordinary_item_is_not_marked_too_large_to_sync() {
        let (state, _dir) = test_state("server");
        match add(&state, 1, "an ordinary clip").data {
            Some(ResponseData::Item(item)) => assert!(!item.too_large_to_sync),
            other => panic!("{other:?}"),
        }
    }

    /// Empty content is a rejected request, not an empty row.
    #[test]
    fn adding_empty_content_is_rejected() {
        let (state, _dir) = test_state("server");

        let response = dispatch_store(
            &state,
            1,
            Method::Add {
                content: "   \n".into(),
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(state.store.count().unwrap(), 0);
    }
}
