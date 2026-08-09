//! `export` and `import` on the wire.
//!
//! The rules — the three skip counts, the `include_sensitive` opt-in, and the
//! detector re-running over every imported item (manifest 04, PG-26) — are
//! [`copypaste_core::transfer`], because Android has no daemon and reaches them
//! by linking the core in-process. What is left here is what only a daemon has:
//! mapping a failure onto a pathless `Response`, and waking the two sync
//! transports once something has actually been written.

use copypaste_ipc::{ErrorCode, ExportItem, Response, ResponseData};
use tracing::warn;

use super::messages::{storage_error, MSG_ENCRYPT, MSG_IMPORT_EMPTY, MSG_IMPORT_TOO_MANY};
use crate::AppState;

pub(super) fn export(state: &AppState, id: u64, limit: u32, include_sensitive: bool) -> Response {
    match copypaste_core::export(&state.store, &state.keyring, limit, include_sensitive) {
        Ok(data) => Response::ok(id, ResponseData::Export(data)),
        Err(e) => storage_error(id, "export", &e),
    }
}

pub(super) fn import(state: &AppState, id: u64, items: Vec<ExportItem>) -> Response {
    let settings = state.settings.get().clone();
    // An import keeps each item's original `created_at` so a restored history
    // keeps its order — which puts every one of them *below* the cloud upload
    // floor on a device that has already synced. Taken before the items move.
    let oldest = items.iter().map(|item| item.created_at).min();
    match copypaste_core::import(
        &state.store,
        &state.detector,
        &state.keyring,
        &settings,
        items,
    ) {
        Ok(result) => {
            if result.inserted > 0 {
                // Without this the imported history is stored, listed, and
                // never offered for upload again: the floor only ever moves
                // forward. `delete`, `delete_all` and `restore` already do it;
                // import was the writer that did not.
                if let Some(oldest) = oldest {
                    crate::cloud::note_version_written(state, oldest);
                    state.p2p.node().note_local_version(oldest);
                }
                state.note_local_change();
            }
            Response::ok(id, ResponseData::Import(result))
        }
        Err(copypaste_core::ImportError::Empty) => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_EMPTY)
        }
        Err(copypaste_core::ImportError::TooMany) => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_TOO_MANY)
        }
        Err(e @ copypaste_core::ImportError::Crypto(_)) => {
            warn!(error = ?e, "import failed to encrypt an item");
            Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
        }
        Err(copypaste_core::ImportError::Storage(e)) => storage_error(id, "import", &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_core::MAX_IMPORT_ITEMS;
    use copypaste_ipc::{ExportData, Method};

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
        match import_of(&target, exported.items).data {
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

    /// The withheld count reaching the wire is what the whole opt-in is for: a
    /// user who is not told believes they exported everything.
    #[test]
    fn a_withheld_item_is_counted_on_the_wire_and_its_content_is_not_there() {
        let (state, _dir) = test_state("alpha");
        crate::testutil::add(&state, "ordinary");
        crate::testutil::add(&state, "AKIAIOSFODNN7EXAMPLE");

        let data = export_of(&state, false);
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.skipped_sensitive, 1);
        assert!(!serde_json::to_string(&data)
            .unwrap()
            .contains("AKIAIOSFODNN7EXAMPLE"));
        assert_eq!(export_of(&state, true).items.len(), 2);
    }

    /// PG-26 through the dispatch path the CLI and the app actually take —
    /// `copypaste_core::transfer` owns the rule, and this is what proves the
    /// handler still routes through it rather than inserting the file's claim.
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

    /// An imported history has to reach the account, and the only thing that
    /// decides what push offers is the upload floor. Every imported stamp is
    /// older than it on a device that has synced once, so an import that leaves
    /// the floor alone is an item stored, listed, and silently never uploaded —
    /// the floor only ever moves forward, so nothing later recovers it.
    #[test]
    fn an_import_pulls_the_cloud_upload_floor_back_to_the_oldest_item() {
        let (state, _dir) = test_state("alpha");
        let after_a_sync = copypaste_core::now_ms();
        state
            .meta
            .set_state_ms(crate::cloud::KEY_UPLOAD_FLOOR, after_a_sync)
            .unwrap();

        let old = 1_700_000_000_000;
        import_of(
            &state,
            vec![
                ExportItem {
                    content: "older".into(),
                    content_type: "text".into(),
                    created_at: old,
                    pinned: false,
                    is_sensitive: false,
                },
                ExportItem {
                    content: "newer".into(),
                    content_type: "text".into(),
                    created_at: old + 60_000,
                    pinned: false,
                    is_sensitive: false,
                },
            ],
        );

        assert_eq!(
            state.meta.state_ms(crate::cloud::KEY_UPLOAD_FLOOR).unwrap(),
            old
        );
        let offered = state.store.versions_since(old, 100).unwrap();
        assert_eq!(offered.len(), 2, "the imported history was not offered");
    }

    /// An import that inserted nothing must not drag the floor back and make
    /// the next round re-offer the whole history for no reason.
    #[test]
    fn an_import_of_items_already_present_leaves_the_floor_where_it_was() {
        let (state, _dir) = test_state("alpha");
        crate::testutil::add(&state, "already here");
        let exported = export_of(&state, false);

        let floor = copypaste_core::now_ms();
        state
            .meta
            .set_state_ms(crate::cloud::KEY_UPLOAD_FLOOR, floor)
            .unwrap();
        match import_of(&state, exported.items).data {
            Some(ResponseData::Import(result)) => assert_eq!(result.inserted, 0),
            other => panic!("{other:?}"),
        }

        assert_eq!(
            state.meta.state_ms(crate::cloud::KEY_UPLOAD_FLOOR).unwrap(),
            floor
        );
    }

    /// Both bounds are refusals a client has to be able to tell from a fault.
    #[test]
    fn an_empty_or_oversized_batch_is_an_invalid_request_and_writes_nothing() {
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
}
