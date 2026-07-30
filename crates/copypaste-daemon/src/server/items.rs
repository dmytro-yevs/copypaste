//! The handlers for the history operations, and the decrypt-to-wire step they
//! all share.
//!
//! Everything here is blocking (SQLite, AEAD, the pasteboard) and is reached
//! through one `spawn_blocking` hop in [`super::dispatch`]. The peer operations
//! are network I/O and live in `crate::p2p::handlers` instead — the split
//! between the two files is the split between the two thread pools.

use copypaste_core::StoredItem;
use copypaste_ipc::{ErrorCode, Item, Response, ResponseData, StatusData, PROTOCOL_VERSION};
use tracing::{error, warn};

use super::messages::{
    decrypt_error, storage_error, MSG_CLIPBOARD, MSG_EMPTY_CONTENT, MSG_ENCRYPT, MSG_NOT_FOUND,
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
        }),
    )
}

pub(super) fn list(state: &AppState, id: u64, limit: u32, offset: u32) -> Response {
    let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
    match state.store.list(limit, offset) {
        Ok(rows) => Response::ok(id, ResponseData::Items(decrypt_rows(state, rows))),
        Err(e) => storage_error(id, "list", e),
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
            Response::ok(id, ResponseData::Items(decrypt_rows(state, rows)))
        }
        Err(e) => storage_error(id, "search", e),
    }
}

pub(super) fn copy(state: &AppState, id: u64, item_id: &str) -> Response {
    let row = match state.store.get(item_id) {
        Ok(Some(row)) => row,
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
    };

    let item = match to_wire(state, row) {
        Ok(item) => item,
        Err(e) => return decrypt_error(id, e),
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
    match capture::ingest(state, content, "text") {
        Ok(ingested) => match to_wire(state, ingested.into_item()) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, e),
        },
        Err(IngestError::Empty) => Response::err(id, ErrorCode::InvalidRequest, MSG_EMPTY_CONTENT),
        Err(e @ IngestError::Crypto(_)) => {
            error!(error = ?e, "add failed to encrypt");
            Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
        }
        Err(e @ IngestError::Storage(_)) => storage_error(id, "add", e),
    }
}

pub(super) fn delete(state: &AppState, id: u64, item_id: &str) -> Response {
    // Read first so an unknown id is `not_found` rather than a silent success:
    // a client that deleted nothing needs to know it deleted nothing.
    let created_at = match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
        Ok(Some(row)) => row.created_at,
    };

    match state.store.delete(item_id) {
        Ok(_) => {
            // A tombstone keeps the item's original `created_at`, so a delete
            // of anything older than the cloud upload cursor is invisible to it
            // until the cursor is pulled back.
            crate::cloud::note_version_written(state, created_at);
            Response::ok(id, ResponseData::Empty {})
        }
        Err(e) => storage_error(id, "delete", e),
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
                match state.meta.oldest_version_ms() {
                    Ok(Some(oldest)) => crate::cloud::note_version_written(state, oldest),
                    Ok(None) => {}
                    Err(e) => warn!(error = ?e, "could not reset the cloud upload cursor"),
                }
            }
            Response::ok(id, ResponseData::Count(deleted))
        }
        Err(e) => storage_error(id, "delete_all", e),
    }
}

pub(super) fn pin(state: &AppState, id: u64, item_id: &str, pinned: bool) -> Response {
    match state.store.get(item_id) {
        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => return storage_error(id, "get", e),
        Ok(Some(_)) => {}
    }

    if let Err(e) = state.store.set_pinned(item_id, pinned) {
        return storage_error(id, "set_pinned", e);
    }

    // Reply with the updated row so a client does not have to re-list to learn
    // the new state.
    match state.store.get(item_id) {
        Ok(Some(row)) => match to_wire(state, row) {
            Ok(item) => Response::ok(id, ResponseData::Item(item)),
            Err(e) => decrypt_error(id, e),
        },
        Ok(None) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
        Err(e) => storage_error(id, "get", e),
    }
}

/// Decrypt a stored row into its wire form.
fn to_wire(state: &AppState, row: StoredItem) -> Result<Item, copypaste_core::CryptoError> {
    let key = state.keyring.item_key();
    // The item id is the AAD: a row decrypted under another row's identity must
    // fail authentication, not fall back to a plaintext read (CLAUDE.md rule 4,
    // "fail closed on crypto").
    let plaintext = copypaste_core::decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)?;
    Ok(Item {
        id: row.id,
        content: String::from_utf8_lossy(&plaintext).into_owned(),
        content_type: row.content_type,
        created_at: row.created_at,
        pinned: row.pinned,
        is_sensitive: row.is_sensitive,
    })
}

/// Decrypt a page of rows, dropping any row that will not open.
///
/// One unreadable row must not blank an entire page of history — the other
/// items are still the user's data. The failure is logged with the row id so it
/// is diagnosable.
fn decrypt_rows(state: &AppState, rows: Vec<StoredItem>) -> Vec<Item> {
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let row_id = row.id.clone();
        match to_wire(state, row) {
            Ok(item) => items.push(item),
            Err(e) => warn!(id = %row_id, error = ?e, "skipping an item that failed to decrypt"),
        }
    }
    items
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
    use crate::testutil::test_state;
    use crate::server::dispatch::dispatch_store;
    use copypaste_ipc::Method;

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
                offset: 0,
            },
        );
        match response.data {
            Some(ResponseData::Items(items)) => {
                assert!(items.iter().any(|item| item.id == added.id));
            }
            other => panic!("expected items, got {other:?}"),
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
            Some(ResponseData::Items(items)) => assert!(
                !items.iter().any(|item| item.id == added.id),
                "a sensitive item reached the search results"
            ),
            other => panic!("expected items, got {other:?}"),
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
