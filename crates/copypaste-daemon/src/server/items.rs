//! The handlers for the history operations, and the decrypt-to-wire step they
//! all share.
//!
//! Everything here is blocking (SQLite, AEAD, the pasteboard) and is reached
//! through one `spawn_blocking` hop in [`super::dispatch`]. The peer operations
//! are network I/O and live in `crate::p2p::handlers` instead — the split
//! between the two files is the split between the two thread pools.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_core::{p2p_contract, ItemCursor, StoredItem};
use copypaste_ipc::{
    clamp_page, ErrorCode, Response, ResponseData, StatusData, DEFAULT_LIST_PAGE,
    DEFAULT_SEARCH_PAGE, MAX_PAGE_CONTENT_BYTES, PROTOCOL_VERSION,
};
use tracing::{error, warn};

mod copy;
mod wire;

pub(super) use self::copy::{copy, copy_plain_text};
use self::wire::{
    bound_item_preview, decrypt_rows, is_oversized_text, to_wire, to_wire_and_payload,
};
use super::messages::{
    decrypt_error, storage_error, MSG_BAD_CURSOR, MSG_CONTENT_TOO_LARGE, MSG_EMPTY_CONTENT,
    MSG_ENCRYPT, MSG_IMAGE_PREVIEW, MSG_NOT_FOUND, MSG_REORDER_TOO_MANY, MSG_TOO_BIG,
};
use crate::capture::{self, IngestError};
use crate::AppState;

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

    let settings = state.settings.get();
    let device_name = state.meta.device_name();
    let listen_addr = state.p2p.listen_addr();
    Response::ok(
        id,
        ResponseData::Status(StatusData {
            device_details: Some(p2p_contract::local_device_details(
                &device_name,
                listen_addr.as_deref(),
            )),
            device_name,
            version: crate::DAEMON_VERSION.to_string(),
            protocol_version: PROTOCOL_VERSION,
            listen_addr,
            item_count,
            capture_running: state.capture_running(),
            clipboard_backend: state.backend_name().to_string(),
            private_mode: settings.private_mode,
            private_mode_epoch: settings.private_mode_epoch(),
            counters: state.counters(),
            // Carried on `status` for the reason the counters are: a client
            // that had to make a second call could render the settings screen
            // before it learned they were not the user's own values.
            settings_health: settings.health().cloned(),
        }),
    )
}

pub(super) fn set_device_name(state: &AppState, id: u64, name: &str) -> Response {
    match state.meta.set_device_name(name) {
        Ok(()) => {
            state.p2p.node().set_device_name(&state.meta.device_name());
            Response::ok(id, ResponseData::Empty {})
        }
        Err(error) => storage_error(id, "rename device", &error),
    }
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

    let page = match state
        .store
        .list_from_bounded(after.as_ref(), limit, MAX_PAGE_CONTENT_BYTES)
    {
        Ok(page) => page,
        Err(e) => return storage_error(id, "list", &e),
    };

    let next = page.next.map(|cursor| cursor.token());
    let mut wire = decrypt_rows(state, page.items);
    for item in &mut wire.items {
        bound_item_preview(item);
    }
    wire.next_cursor = next;
    Response::ok(id, ResponseData::Page(wire))
}

pub(super) fn search(state: &AppState, id: u64, query: &str, limit: u32) -> Response {
    let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
    match state.store.search(query, limit) {
        Ok(rows) => {
            // Read-time enforcement of "sensitive items are never searchable".
            // The store already keeps them out of the index at write time; this
            // is the second of the three layers the rule demands, and it is
            // what protects a database written before the rule existed.
            let mut rows: Vec<StoredItem> =
                rows.into_iter().filter(|row| !row.is_sensitive).collect();
            // Search carries no cursor, so matches past the budget are dropped
            // rather than deferred. An undeliverable frame would drop all of them.
            rows.truncate(within_budget(&rows));
            let mut page = decrypt_rows(state, rows);
            for item in &mut page.items {
                bound_item_preview(item);
            }
            Response::ok(id, ResponseData::Page(page))
        }
        Err(e) => storage_error(id, "search", &e),
    }
}

/// Produce a small PNG only when a history row needs to draw an image.
///
/// This is deliberately not part of `list`: decrypting every screenshot in a
/// long history would spend memory and expose data the user has not viewed.
pub(super) fn image_preview(state: &AppState, id: u64, item_id: &str) -> Response {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(error) => return storage_error(id, "image_preview", &error),
    };
    if row.is_sensitive || !row.content_type.starts_with("image/") {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_IMAGE_PREVIEW);
    }
    let bytes = match copypaste_core::open_binary(
        &row.content_ciphertext,
        &state.keyring.item_key(),
        &row.id,
    ) {
        Ok(bytes) => bytes,
        Err(error) => return decrypt_error(id, &error),
    };
    let budget = state.settings.get().max_decoded_image_mb;
    let thumbnail = match copypaste_core::thumbnail_png(&bytes, budget) {
        Ok(thumbnail) => thumbnail,
        Err(error) => {
            warn!(id = %row.id, error = ?error, "image preview unavailable");
            return Response::err(id, ErrorCode::InvalidRequest, MSG_IMAGE_PREVIEW);
        }
    };
    Response::ok(
        id,
        ResponseData::ImagePreview(copypaste_ipc::ImagePreview {
            png_base64: STANDARD.encode(thumbnail.png),
            width: thumbnail.width,
            height: thumbnail.height,
        }),
    )
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
        Ok(Some(row)) => match to_wire_and_payload(state, row) {
            Ok((_, payload)) if is_oversized_text(&payload) => {
                Response::err(id, ErrorCode::ContentTooLarge, MSG_CONTENT_TOO_LARGE)
            }
            Ok((item, _)) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, &e),
        },
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => storage_error(id, "get", &e),
    }
}

pub(super) fn delete(state: &AppState, id: u64, item_id: &str) -> Response {
    // Read first so an unknown id is `not_found` rather than a silent success:
    // a client that deleted nothing needs to know it deleted nothing.
    match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", &e),
        Ok(Some(_)) => {}
    };

    let mutation_started = copypaste_core::now_ms();
    match state.store.delete(item_id) {
        Ok(_) => {
            crate::cloud::note_version_written(state, mutation_started);
            state.note_local_change();
            Response::ok(id, ResponseData::Empty {})
        }
        Err(e) => storage_error(id, "delete", &e),
    }
}

pub(super) fn delete_all(state: &AppState, id: u64, through: Option<i64>) -> Response {
    // Manifest 03 (`CopyPaste-cb7u`) has `delete_all` tombstone only the
    // non-pinned rows — a pin is the user saying "keep this". The predicate
    // lives in `Store::delete_all_through` and is deliberately not repeated
    // here: filtering in the server would put one manifest rule in two places.
    let mutation_started = copypaste_core::now_ms();
    let result = match through {
        Some(through) => state.store.delete_all_through(through),
        None => state.store.delete_all(),
    };
    match result {
        Ok(deleted) => {
            if deleted > 0 {
                crate::cloud::note_version_written(state, mutation_started);
                state.note_local_change();
            }
            Response::ok(id, ResponseData::Count(deleted))
        }
        Err(e) => storage_error(id, "delete_all", &e),
    }
}

pub(super) fn history_ceiling(state: &AppState, id: u64) -> Response {
    match state.store.max_rowid() {
        Ok(ceiling) => Response::ok(id, ResponseData::Count(ceiling.max(0) as u64)),
        Err(e) => storage_error(id, "history_ceiling", &e),
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
        Ok(Some(row)) => match to_wire_and_payload(state, row) {
            Ok((mut item, payload)) => {
                if is_oversized_text(&payload) {
                    bound_item_preview(&mut item);
                }
                Response::ok(id, ResponseData::Item(item))
            }
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

/// How many of `rows`, in order, fit [`MAX_PAGE_CONTENT_BYTES`].
///
/// Never zero while there are rows: an item at the ceiling has to be a page of
/// its own or the list stops there for good. Ciphertext is the measure because
/// it is what is in hand before decrypting, and it only ever exceeds the
/// plaintext it stands for.
fn within_budget(rows: &[StoredItem]) -> usize {
    let mut bytes = 0usize;
    for (n, row) in rows.iter().enumerate() {
        bytes = bytes.saturating_add(row.content_ciphertext.len());
        if bytes > MAX_PAGE_CONTENT_BYTES {
            return n.max(1);
        }
    }
    rows.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::dispatch::dispatch_store;
    use crate::testutil::test_state;
    use base64::engine::general_purpose::STANDARD;
    use copypaste_ipc::limits::LIST_PREVIEW_BYTES;
    use copypaste_ipc::{content_type::TEXT, Method};

    fn seed_legacy_text(state: &AppState, id: &str, content: &str, content_type: &str) {
        let key = state.keyring.item_key();
        let (nonce, content_ciphertext) = copypaste_core::encrypt(content.as_bytes(), &key, id)
            .expect("the current item key seals the direct legacy row");
        state
            .store
            .insert(copypaste_core::NewItem {
                id: id.to_string(),
                content_ciphertext,
                nonce,
                content_type: content_type.to_string(),
                content_hash: copypaste_core::compute_content_hash(content.as_bytes()),
                is_sensitive: false,
                search_text: Some(content.to_string()),
                created_at: copypaste_core::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .expect("a pre-limit row is still a valid stored row");
    }

    fn row_of(bytes: usize) -> StoredItem {
        StoredItem {
            id: String::new(),
            content_ciphertext: vec![0; bytes],
            nonce: Vec::new(),
            content_type: TEXT.to_string(),
            content_hash: String::new(),
            created_at: 0,
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
            is_sensitive: false,
            deleted: false,
            origin_device_id: String::new(),
            app_bundle_id: None,
            app_name: None,
            payload_metadata: None,
        }
    }

    fn binary_item(
        state: &AppState,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> StoredItem {
        copypaste_core::ingest_binary_into_with_capture_context(
            &state.store,
            &state.keyring,
            bytes,
            content_type,
            copypaste_core::now_ms(),
            false,
            None,
            metadata,
            &state.settings.get(),
        )
        .unwrap()
        .into_item()
    }

    /// `MAX_PAGE` bounds a page's count and nothing about its size, so a page
    /// of large items serialised past the frame cap and reached the client as a
    /// decode error that took every item beside it down.
    #[test]
    fn a_page_stops_at_the_byte_budget_not_only_the_row_count() {
        let half = MAX_PAGE_CONTENT_BYTES / 2 + 1;
        let three = [row_of(half), row_of(half), row_of(half)];
        assert_eq!(within_budget(&three), 1);
        assert_eq!(within_budget(&[row_of(8), row_of(8)]), 2);
    }

    /// An item at the ceiling has to be a page of its own: returning zero would
    /// hand back an empty page with a cursor that never advances, and the list
    /// would stop there for good.
    #[test]
    fn one_item_over_the_budget_is_still_served() {
        assert_eq!(within_budget(&[row_of(MAX_PAGE_CONTENT_BYTES * 2)]), 1);
    }

    #[test]
    fn an_over_budget_page_resumes_at_the_first_item_it_dropped() {
        const BIG: usize = 1_500_000;

        let (state, _dir) = test_state("list-over-budget");
        let mut added = Vec::new();
        for n in 0..3 {
            let body = format!("{n:07} ").repeat(BIG / 8);
            match add(&state, n as u64, &body).data {
                Some(ResponseData::Item(item)) => added.push(item.id),
                other => panic!("{other:?}"),
            }
        }

        let first = match list(&state, 1, 10, None).data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            first.items.len(),
            2,
            "the byte budget did not trim the page"
        );
        let cursor = first
            .next_cursor
            .expect("a trimmed page must carry a cursor");

        let second = match list(&state, 2, 10, Some(&cursor)).data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };

        let mut seen: Vec<String> = first.items.iter().map(|i| i.id.clone()).collect();
        seen.extend(second.items.iter().map(|i| i.id.clone()));
        let mut sorted = seen.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), seen.len(), "a row was served twice: {seen:?}");
        added.sort();
        assert_eq!(sorted, added, "paging did not visit every item");
    }

    /// I-39 / §6.5: the two clipboard counters existed and nothing read them,
    /// which is the same as not having them. This is the caller that made
    /// deleting their `allow(dead_code)` correct.
    #[test]
    fn a_full_page_of_large_clippings_serialises_within_the_preview_bound() {
        const PAGE: usize = 200;
        const BODY: usize = 2048;

        let (state, _dir) = test_state("list-page-bytes");
        for n in 0..PAGE {
            let body = format!("{n:07} ").repeat(BODY / 8);
            assert!(body.len() >= BODY);
            match add(&state, n as u64, &body).data {
                Some(ResponseData::Item(_)) => {}
                other => panic!("{other:?}"),
            }
        }

        let response = list(&state, 1, PAGE as u32, None);
        let bounded = serde_json::to_string(&response).unwrap().len();

        let mut whole = match response.data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };
        assert_eq!(whole.items.len(), PAGE);
        assert!(whole.items.iter().all(|item| item.truncated));
        assert!(whole.items.iter().all(|item| {
            item.sensitive_finding.as_ref().is_none_or(|finding| {
                finding.spans.len() <= copypaste_core::sensitive::MAX_SURFACED_SENSITIVE_SPANS
            })
        }));
        for item in &mut whole.items {
            item.content = item.content.repeat(BODY / LIST_PREVIEW_BYTES);
            item.truncated = false;
        }
        let unbounded = serde_json::to_string(&Response::ok(1, ResponseData::Page(whole)))
            .unwrap()
            .len();

        let item_bound = LIST_PREVIEW_BYTES * 2
            + copypaste_core::sensitive::MAX_SURFACED_SENSITIVE_SPANS * 48
            + 768;
        assert!(
            bounded <= PAGE * item_bound,
            "a bounded page of {PAGE} items serialised to {bounded} bytes"
        );
        assert!(
            unbounded > bounded,
            "{unbounded} was not larger than {bounded}"
        );
        eprintln!("page bytes: bounded {bounded}, whole bodies {unbounded}");
    }

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

    #[test]
    fn a_renamed_device_identity_survives_reopening() {
        let (state, _dir) = test_state("old name");
        assert!(set_device_name(&state, 1, "  Kitchen Mac  ").ok);

        let reopened = crate::meta::Meta::open(&state.store, "ignored hostname").unwrap();
        assert_eq!(reopened.device_id(), state.meta.device_id());
        assert_eq!(reopened.device_name(), "Kitchen Mac");
        match status(&state, 2).data {
            Some(ResponseData::Status(status)) => assert_eq!(status.device_name, "Kitchen Mac"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_blank_device_name_is_refused_without_changing_the_identity() {
        let (state, _dir) = test_state("Office Mac");
        let response = set_device_name(&state, 1, " \n\t ");
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(state.meta.device_name(), "Office Mac");
    }

    /// Rule 4. `status` is the one reply a support flow is built on, so a path
    /// reaching it would be pasted into every issue.
    #[test]
    fn status_never_carries_a_path() {
        let (state, _dir) = test_state("status-paths");
        let json = serde_json::to_string(&status(&state, 1)).unwrap();
        assert_eq!(json, copypaste_ipc::redact::scrub_paths(&json), "{json}");
    }

    /// A sensitive item is stored, is visible in `list`, and is never returned
    /// by `search`.
    ///
    /// The store keeps it out of the FTS index at write time; this covers the
    /// server's read-time layer, which is what protects a database written
    /// before the rule existed (AGENTS.md rule 4 — "enforced at write time, at
    /// read time, and by a purge migration").
    /// Reading must never have the side effect of copying: `get` returns the
    /// content, and the clipboard is untouched by it.
    #[test]
    fn list_bounds_bodies_while_get_and_copy_still_answer_in_full() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("list-preview");
        let long = "x".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES * 3);
        let added = match add(&state, 1, &long).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        assert_eq!(added.content.len(), long.len());
        assert!(!added.truncated);

        match list(&state, 2, 50, None).data {
            Some(ResponseData::Page(page)) => {
                let item = page
                    .items
                    .iter()
                    .find(|candidate| candidate.id == added.id)
                    .expect("the item is listed");
                assert_eq!(
                    item.content.len(),
                    copypaste_ipc::limits::LIST_PREVIEW_BYTES
                );
                assert!(item.truncated);
                assert!(long.starts_with(&item.content));
            }
            other => panic!("{other:?}"),
        }

        match get(&state, 3, &added.id).data {
            Some(ResponseData::Item(item)) => {
                assert_eq!(item.content, long);
                assert!(!item.truncated);
            }
            other => panic!("{other:?}"),
        }

        match copy(&state, 4, &added.id).data {
            Some(ResponseData::Item(item)) => assert_eq!(item.content, long),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            writes.entries(),
            vec![crate::testutil::WrittenPayload::Text(long)],
            "copy wrote the list preview instead of the authenticated full body"
        );
    }

    #[test]
    fn authenticated_legacy_text_refuses_full_paths_but_keeps_a_safe_preview() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("legacy-text");
        let body = format!(
            "needle {}",
            "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1 - "needle ".len())
        );
        seed_legacy_text(&state, "legacy-plus-one", &body, TEXT);
        let before = state
            .store
            .get("legacy-plus-one")
            .unwrap()
            .unwrap()
            .content_ciphertext;
        let source = state.store.get("legacy-plus-one").unwrap().unwrap();
        state
            .store
            .insert(copypaste_core::NewItem {
                id: "legacy-wrong-aad".into(),
                content_ciphertext: source.content_ciphertext.clone(),
                nonce: source.nonce.clone(),
                content_type: TEXT.into(),
                content_hash: "wrong-aad-content-hash".into(),
                is_sensitive: false,
                search_text: None,
                created_at: source.created_at.saturating_sub(1),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .unwrap();
        for response in [
            get(&state, 0, "legacy-wrong-aad"),
            copy(&state, 0, "legacy-wrong-aad"),
        ] {
            assert!(!response.ok);
            assert_eq!(response.error_code, Some(ErrorCode::Internal));
            assert!(response.data.is_none());
        }
        assert_eq!(writes.count(), 0);

        for response in [
            get(&state, 1, "legacy-plus-one"),
            copy(&state, 2, "legacy-plus-one"),
            copy_plain_text(&state, 3, "legacy-plus-one"),
        ] {
            assert!(!response.ok);
            assert_eq!(response.error_code, Some(ErrorCode::ContentTooLarge));
            assert!(response.data.is_none());
        }
        assert_eq!(writes.count(), 0);

        let listed = match list(&state, 4, 20, None).data {
            Some(ResponseData::Page(page)) => page,
            _ => panic!("list must return a page"),
        };
        assert!(listed.items[0].truncated);
        assert!(listed.items[0].content.len() <= LIST_PREVIEW_BYTES);
        assert!(serde_json::to_string(&listed).unwrap().len() <= copypaste_ipc::MAX_FRAME_BYTES);

        let searched = match search(&state, 5, "needle", 20).data {
            Some(ResponseData::Page(page)) => page,
            _ => panic!("search must return a page"),
        };
        assert!(searched.items[0].truncated);
        assert!(searched.items[0].content.len() <= LIST_PREVIEW_BYTES);

        for pinned in [true, false] {
            let response = pin(&state, 6, "legacy-plus-one", pinned);
            assert!(response.ok);
            let item = match response.data {
                Some(ResponseData::Item(item)) => item,
                _ => panic!("pin must return its preview"),
            };
            assert!(item.truncated);
            assert!(item.content.len() <= LIST_PREVIEW_BYTES);
            assert!(serde_json::to_string(&item).unwrap().len() <= copypaste_ipc::MAX_FRAME_BYTES);
        }
        for (index, content_type) in [
            TEXT,
            "text/plain",
            copypaste_ipc::content_type::RICH_TEXT,
            copypaste_ipc::content_type::HTML,
        ]
        .into_iter()
        .enumerate()
        {
            let id = format!("legacy-text-type-{index}");
            let typed_body = format!(
                "{index}{}",
                "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES)
            );
            seed_legacy_text(&state, &id, &typed_body, content_type);
            let response = get(&state, 7, &id);
            assert_eq!(response.error_code, Some(ErrorCode::ContentTooLarge));
            assert!(response.data.is_none());
        }
        assert_eq!(
            state
                .store
                .get("legacy-plus-one")
                .unwrap()
                .unwrap()
                .content_ciphertext,
            before
        );
    }

    #[test]
    fn an_exact_limit_legacy_text_still_has_a_full_frame_safe_response() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("legacy-exact");
        let body = "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES);
        seed_legacy_text(&state, "legacy-exact", &body, "text/plain");

        let response = get(&state, 1, "legacy-exact");
        assert!(response.ok);
        assert!(serde_json::to_string(&response).unwrap().len() <= copypaste_ipc::MAX_FRAME_BYTES);
        assert_eq!(writes.count(), 0);
    }

    #[test]
    fn pinning_an_ordinary_body_keeps_its_full_response() {
        let (state, _dir) = test_state("ordinary-pin-response");
        let body = "x".repeat(LIST_PREVIEW_BYTES * 2);
        let item = match add(&state, 1, &body).data {
            Some(ResponseData::Item(item)) => item,
            _ => panic!("add must return an item"),
        };

        for pinned in [true, false] {
            let response = pin(&state, 2, &item.id, pinned);
            let updated = match response.data {
                Some(ResponseData::Item(item)) => item,
                _ => panic!("pin must return its item"),
            };
            assert_eq!(updated.content.len(), body.len());
            assert!(!updated.truncated);
        }
    }

    #[test]
    fn a_body_within_the_bound_is_listed_whole_and_unmarked() {
        let (state, _dir) = test_state("list-preview-short");
        let added = match add(&state, 1, "short enough").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        match list(&state, 2, 50, None).data {
            Some(ResponseData::Page(page)) => {
                let item = page
                    .items
                    .iter()
                    .find(|candidate| candidate.id == added.id)
                    .expect("the item is listed");
                assert_eq!(item.content, "short enough");
                assert!(!item.truncated);
            }
            other => panic!("{other:?}"),
        }
    }

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

        assert!(copy_plain_text(&state, 4, &added.id).ok);
        assert_eq!(
            writes.count(),
            2,
            "plain-text copy did not reach the clipboard"
        );

        // An unknown id is not_found, not an empty success.
        let missing = get(&state, 5, "00000000-0000-0000-0000-000000000000");
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
        assert!(added.sensitive_finding.is_none());

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

    #[test]
    fn inert_findings_are_redacted_for_display_but_remain_searchable() {
        let (state, _dir) = test_state("inert-sensitive-finding");
        let text = "mail alice@example.com about the release";
        let added = match add(&state, 1, text).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("expected an item, got {other:?}"),
        };

        assert!(!added.is_sensitive);
        let finding = added.sensitive_finding.as_ref().unwrap();
        assert_eq!(finding.label, "email");
        assert_eq!(finding.spans.len(), 1);
        assert!(!finding.redacted_preview.contains("alice@example.com"));

        match search(&state, 2, "alice", 20).data {
            Some(ResponseData::Page(page)) => {
                assert!(page.items.iter().any(|item| item.id == added.id));
            }
            other => panic!("expected a page, got {other:?}"),
        }
        assert!(state
            .store
            .summaries(20)
            .unwrap()
            .iter()
            .any(|version| version.id == added.id));
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

    #[test]
    fn image_preview_is_lazy_bounded_and_never_the_original_payload() {
        let (state, _dir) = test_state("image-preview");
        let source = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        let image = copypaste_core::ingest_binary_into_with_capture_context(
            &state.store,
            &state.keyring,
            &source,
            copypaste_ipc::content_type::IMAGE_PNG,
            copypaste_core::now_ms(),
            false,
            None,
            None,
            &state.settings.get(),
        )
        .unwrap()
        .into_item();

        let listed = match list(&state, 1, 20, None).data {
            Some(ResponseData::Page(page)) => page.items,
            other => panic!("{other:?}"),
        };
        assert_eq!(listed[0].content, "[image]");

        let preview = match image_preview(&state, 2, &image.id).data {
            Some(ResponseData::ImagePreview(preview)) => preview,
            other => panic!("{other:?}"),
        };
        assert_eq!((preview.width, preview.height), (1, 1));
        assert_eq!(
            &STANDARD.decode(preview.png_base64).unwrap()[..8],
            b"\x89PNG\r\n\x1a\n"
        );
    }

    #[test]
    fn native_copy_dispatches_bytes_while_plain_text_copy_refuses_display_labels() {
        use crate::testutil::WrittenPayload;

        let (state, _dir, writes) =
            crate::testutil::test_state_watching_clipboard("native-copy-payloads");
        let image_bytes = b"native image bytes";
        let image = binary_item(
            &state,
            copypaste_ipc::content_type::IMAGE_PNG,
            image_bytes,
            None,
        );
        let metadata =
            copypaste_core::FileMetadata::new("report.bin", "application/octet-stream").unwrap();
        let file_bytes = b"native file bytes";
        let file = binary_item(
            &state,
            copypaste_ipc::content_type::FILE,
            file_bytes,
            Some(&metadata),
        );
        let unknown = binary_item(&state, "application/x-future", b"future bytes", None);

        assert!(copy(&state, 1, &image.id).ok);
        assert!(copy(&state, 2, &file.id).ok);
        assert_eq!(
            writes.entries(),
            vec![
                WrittenPayload::Image(image_bytes.to_vec()),
                WrittenPayload::File {
                    bytes: file_bytes.to_vec(),
                    metadata: Some(metadata),
                },
            ]
        );

        for (request, item_id) in [(3, &image.id), (4, &file.id), (5, &unknown.id)] {
            let response = copy_plain_text(&state, request, item_id);
            assert_eq!(
                response.error_code,
                Some(ErrorCode::UnsupportedContent),
                "{item_id}"
            );
        }
        let unknown_native = copy(&state, 6, &unknown.id);
        assert_eq!(
            unknown_native.error_code,
            Some(ErrorCode::UnsupportedContent)
        );
        assert_eq!(writes.count(), 2, "a display placeholder was written");

        let listed = match list(&state, 7, 20, None).data {
            Some(ResponseData::Page(page)) => page.items,
            other => panic!("{other:?}"),
        };
        for expected in ["[image]", "[file]", "[unsupported]"] {
            assert!(
                listed.iter().any(|item| item.content == expected),
                "{expected}"
            );
        }
    }

    #[test]
    fn image_preview_refuses_a_sensitive_item() {
        let (state, _dir) = test_state("image-preview-sensitive");
        let image = copypaste_core::ingest_binary_into_with_capture_context(
            &state.store,
            &state.keyring,
            b"bytes",
            copypaste_ipc::content_type::IMAGE_PNG,
            copypaste_core::now_ms(),
            true,
            None,
            None,
            &state.settings.get(),
        )
        .unwrap()
        .into_item();

        let response = image_preview(&state, 1, &image.id);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(response.error.as_deref(), Some(MSG_IMAGE_PREVIEW));
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
            Some(state.meta.device_name().as_str())
        );

        // A row that arrived from elsewhere must not read as local, and it has
        // no name until a session with that device has told us one. Applied
        // through the real merge path a peer's item takes, because that is what
        // stamps `origin_device_id` on the row.
        crate::sync::store_source(&state)
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "from-the-phone",
                content: "arrived over the peer transport",
                binary_content: None,
                payload_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT,
                created_at: copypaste_core::now_ms(),
                deleted: false,
                content_hash: None,
                origin_device_id: "device-b",
                app_bundle_id: None,
                app_name: None,
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
                    Some(state.meta.device_name().as_str())
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

    #[test]
    fn a_delete_pulls_the_cloud_upload_floor_back_to_the_tombstone() {
        let (state, _dir) = test_state("server");
        let added = match add(&state, 1, "to delete").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        let ahead = added.created_at.saturating_add(60_000);
        state
            .meta
            .set_state_all(&[
                (crate::cloud::KEY_UPLOAD_FLOOR, &ahead.to_string()),
                (crate::cloud::KEY_UPLOAD_FLOOR_ITEM, "zzzz"),
            ])
            .unwrap();

        assert!(delete(&state, 2, &added.id).ok);

        let floor = state.meta.state_ms(crate::cloud::KEY_UPLOAD_FLOOR).unwrap();
        assert!(
            floor <= ahead,
            "delete left the upload floor ahead of the tombstone"
        );
        let offered = state.store.versions_since(floor, 100).unwrap();
        assert!(
            offered.iter().any(|row| row.id == added.id && row.deleted),
            "the tombstone was not offered above the floor"
        );
    }

    #[test]
    fn delete_all_pulls_the_cloud_upload_floor_back() {
        let (state, _dir) = test_state("server");
        assert!(add(&state, 1, "clear me").ok);
        let ahead = copypaste_core::now_ms().saturating_add(60_000);
        state
            .meta
            .set_state_ms(crate::cloud::KEY_UPLOAD_FLOOR, ahead)
            .unwrap();

        assert!(delete_all(&state, 2, None).ok);

        let floor = state.meta.state_ms(crate::cloud::KEY_UPLOAD_FLOOR).unwrap();
        assert!(floor < ahead, "delete_all left the upload floor ahead");
        assert!(
            !state.store.versions_since(floor, 100).unwrap().is_empty(),
            "cleared tombstones were not offered"
        );
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
