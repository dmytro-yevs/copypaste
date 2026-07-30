//! Getting history out of the daemon and back in.
//!
//! # An export says what it left out
//!
//! Three counts come back beside the items, always, including when they are
//! zero. v1 shipped `skipped_non_text` for exactly this reason
//! (`CopyPaste-93yr`): a file that quietly contains fewer items than the history
//! it was taken from is worse than one that says so, because the user only
//! finds out when they need it.
//!
//! # Sensitive items are excluded by default
//!
//! An export is plaintext and leaves the app's control the moment it is written.
//! `include_sensitive` defaults to false on the wire and in every client, and a
//! withheld item is *counted*, so "why is this shorter" has an answer
//! (P2-tj9s).
//!
//! # An import is an ingest, not an insert
//!
//! Every item goes through [`crate::capture::ingest_at`] — the same path a
//! capture takes — so the detector runs again over the plaintext and the result
//! is OR-ed with whatever the file claimed. A hand-edited export cannot
//! reintroduce a credential marked clean (manifest 04, PG-26). A malformed
//! batch is refused whole, before anything is written.

use copypaste_ipc::{ErrorCode, ExportData, ExportItem, ImportData, Response, ResponseData};
use tracing::{info, warn};

use super::messages::{storage_error, MSG_IMPORT_EMPTY, MSG_IMPORT_TOO_MANY};
use crate::capture::{ingest_at, IngestError, Ingested};
use crate::AppState;

/// How many rows one export reads out of the store at a time.
const EXPORT_CHUNK: u32 = 500;

/// Ceiling on one import batch. The frame cap bounds the bytes; this bounds the
/// work, so a single request cannot hold the database mutex for minutes.
const MAX_IMPORT_ITEMS: usize = 10_000;

pub(super) fn export(state: &AppState, id: u64, limit: u32, include_sensitive: bool) -> Response {
    let key = state.keyring.item_key();
    let mut data = ExportData {
        items: Vec::new(),
        skipped_non_text: 0,
        skipped_sensitive: 0,
        skipped_undecryptable: 0,
    };

    // Paged rather than one unbounded `list`: a full history is 10 000 rows of
    // plaintext and this holds one page of ciphertext at a time on the way to
    // holding all of the plaintext anyway — but the query stays cheap and the
    // limit stops early.
    let mut offset = 0u32;
    loop {
        let rows = match state.store.list(EXPORT_CHUNK, offset) {
            Ok(rows) => rows,
            Err(e) => return storage_error(id, "export", &e),
        };
        if rows.is_empty() {
            break;
        }
        offset = offset.saturating_add(EXPORT_CHUNK);

        for row in rows {
            if row.is_sensitive && !include_sensitive {
                data.skipped_sensitive += 1;
                continue;
            }
            if !is_text(&row.content_type) {
                data.skipped_non_text += 1;
                continue;
            }
            let plaintext = match copypaste_core::decrypt(
                &row.content_ciphertext,
                &row.nonce,
                &key,
                &row.id,
            ) {
                Ok(plaintext) => plaintext,
                Err(e) => {
                    warn!(id = %row.id, error = ?e, "export skipped an item that failed to decrypt");
                    data.skipped_undecryptable += 1;
                    continue;
                }
            };
            data.items.push(ExportItem {
                content: String::from_utf8_lossy(&plaintext).into_owned(),
                content_type: row.content_type,
                created_at: row.created_at,
                pinned: row.pinned,
                is_sensitive: row.is_sensitive,
            });
            if limit > 0 && data.items.len() as u32 >= limit {
                return finish(id, data);
            }
        }
    }

    finish(id, data)
}

fn finish(id: u64, data: ExportData) -> Response {
    // The count only — never content, and never a sample.
    info!(
        items = data.items.len(),
        skipped_sensitive = data.skipped_sensitive,
        skipped_non_text = data.skipped_non_text,
        skipped_undecryptable = data.skipped_undecryptable,
        "exported history"
    );
    Response::ok(id, ResponseData::Export(data))
}

pub(super) fn import(state: &AppState, id: u64, items: Vec<ExportItem>) -> Response {
    if items.is_empty() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_EMPTY);
    }
    if items.len() > MAX_IMPORT_ITEMS {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_TOO_MANY);
    }

    let mut result = ImportData {
        inserted: 0,
        skipped: 0,
    };
    let mut pin: Vec<String> = Vec::new();

    for item in items {
        // The caller's flag is a floor: `ingest_at` recomputes sensitivity from
        // the plaintext, so this can only ever add to what the detector found.
        match ingest_at(state, &item.content, &item.content_type, item.created_at) {
            Ok(Ingested::Stored(stored)) => {
                result.inserted += 1;
                if item.pinned {
                    pin.push(stored.id);
                }
            }
            Ok(Ingested::Duplicate(existing)) => {
                result.skipped += 1;
                if item.pinned {
                    pin.push(existing.id);
                }
            }
            // Empty and over-size are the two entries worth tolerating: neither
            // can be stored and neither is a reason to lose the other 9 999.
            Err(IngestError::Empty | IngestError::TooLarge) => result.skipped += 1,
            Err(e @ IngestError::Crypto(_)) => {
                warn!(error = ?e, "import failed to encrypt an item");
                return Response::err(id, ErrorCode::Internal, super::messages::MSG_ENCRYPT);
            }
            Err(IngestError::Storage(e)) => return storage_error(id, "import", &e),
        }
    }

    // Pins are applied after the fact rather than inside the ingest: the store
    // has one pin path (`set_pinned`, which also assigns `pin_order`) and a
    // second one here would be a second definition of what a pin is.
    for item_id in &pin {
        if let Err(e) = state.store.set_pinned(item_id, true) {
            warn!(error = ?e, "could not restore a pin on an imported item");
        }
    }

    if result.inserted > 0 {
        state.note_local_change();
    }
    info!(
        inserted = result.inserted,
        skipped = result.skipped,
        "imported history"
    );
    Response::ok(id, ResponseData::Import(result))
}

/// v2 captures text only, but a peer or the cloud can deliver another type.
fn is_text(content_type: &str) -> bool {
    copypaste_ipc::content_type::is_text(content_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::Method;

    fn export_of(state: &AppState, include_sensitive: bool) -> ExportData {
        match export(state, 1, 0, include_sensitive).data {
            Some(ResponseData::Export(data)) => data,
            other => panic!("{other:?}"),
        }
    }

    fn import_of(state: &AppState, items: Vec<ExportItem>) -> Response {
        crate::server::dispatch::dispatch_store(state, 2, Method::Import { items })
    }

    #[test]
    fn an_export_round_trips_through_an_import_on_a_fresh_daemon() {
        let (source, _a) = test_state("alpha");
        crate::testutil::add(&source, "first");
        crate::testutil::add(&source, "second");
        let exported = export_of(&source, false);
        assert_eq!(exported.items.len(), 2);

        let (target, _b) = test_state("beta");
        let response = import_of(&target, exported.items.clone());
        match response.data {
            Some(ResponseData::Import(result)) => {
                assert_eq!(result.inserted, 2);
                assert_eq!(result.skipped, 0);
            }
            other => panic!("{other:?}"),
        }
        let mut round_tripped = crate::testutil::contents(&target);
        round_tripped.sort();
        assert_eq!(round_tripped, ["first", "second"]);
    }

    /// The property the whole module is shaped around: nothing is dropped
    /// silently.
    #[test]
    fn a_sensitive_item_is_withheld_by_default_and_counted() {
        let (state, _dir) = test_state("alpha");
        crate::testutil::add(&state, "ordinary");
        crate::testutil::add(&state, "AKIAIOSFODNN7EXAMPLE");

        let default = export_of(&state, false);
        assert_eq!(default.items.len(), 1);
        assert_eq!(default.skipped_sensitive, 1);
        let rendered = serde_json::to_string(&default).unwrap();
        assert!(
            !rendered.contains("AKIAIOSFODNN7EXAMPLE"),
            "a withheld item's content reached the export"
        );

        // ...and it is present only when asked for explicitly.
        let opted_in = export_of(&state, true);
        assert_eq!(opted_in.items.len(), 2);
        assert_eq!(opted_in.skipped_sensitive, 0);
    }

    /// Every count is on the wire even when it is zero: a field that appears
    /// only when non-zero is one nobody knows to look for.
    #[test]
    fn the_skip_counts_are_always_present() {
        let (state, _dir) = test_state("alpha");
        crate::testutil::add(&state, "ordinary");
        let json = serde_json::to_value(export_of(&state, false)).unwrap();
        for field in [
            "skipped_non_text",
            "skipped_sensitive",
            "skipped_undecryptable",
        ] {
            assert_eq!(json[field], 0, "{field} missing or wrong");
        }
    }

    /// PG-26. An export edited to claim a credential is clean must not import
    /// as clean: the detector is re-run and the two are OR-ed.
    #[test]
    fn an_import_cannot_smuggle_a_credential_in_marked_clean() {
        let (state, _dir) = test_state("alpha");
        import_of(
            &state,
            vec![ExportItem {
                content: "AKIAIOSFODNN7EXAMPLE".into(),
                content_type: "text".into(),
                created_at: 1_700_000_000_000,
                pinned: false,
                is_sensitive: false,
            }],
        );
        let row = state.store.list(10, 0).unwrap().remove(0);
        assert!(row.is_sensitive, "the detector did not re-run on import");
        assert!(
            state
                .store
                .search("AKIAIOSFODNN7EXAMPLE", 10)
                .unwrap()
                .is_empty(),
            "an imported credential reached the search index"
        );
    }

    #[test]
    fn timestamps_and_pins_survive_a_round_trip() {
        let (source, _a) = test_state("alpha");
        let id = crate::testutil::add(&source, "keep me");
        source.store.set_pinned(&id, true).unwrap();
        let created_at = source.store.get(&id).unwrap().unwrap().created_at;

        let (target, _b) = test_state("beta");
        import_of(&target, export_of(&source, false).items);
        let row = target.store.list(10, 0).unwrap().remove(0);
        assert_eq!(row.created_at, created_at, "the original stamp was lost");
        assert!(row.pinned, "the pin was lost");
    }

    #[test]
    fn an_empty_or_oversized_batch_is_refused_before_anything_is_written() {
        let (state, _dir) = test_state("alpha");
        assert_eq!(
            import_of(&state, Vec::new()).error_code,
            Some(ErrorCode::InvalidRequest)
        );

        let item = ExportItem {
            content: "x".into(),
            content_type: "text".into(),
            created_at: 1,
            pinned: false,
            is_sensitive: false,
        };
        let response = import_of(&state, vec![item; MAX_IMPORT_ITEMS + 1]);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(state.store.count().unwrap(), 0);
    }

    #[test]
    fn a_non_text_item_is_skipped_and_counted_rather_than_dropped_quietly() {
        assert!(is_text("text"));
        assert!(is_text("text/plain"));
        assert!(!is_text("image/png"));
    }
}
