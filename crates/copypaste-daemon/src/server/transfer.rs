//! `export` and `import` on the wire.
//!
//! The rules for export and import are
//! [`copypaste_core::transfer`], because Android has no daemon and reaches them
//! by linking the core in-process. What is left here is what only a daemon has:
//! mapping a failure onto a pathless `Response`, and waking the two sync
//! transports once something has actually been written.

use copypaste_ipc::{ErrorCode, ExportItem, Response, ResponseData};
use tracing::warn;

use super::messages::{storage_error, MSG_ENCRYPT, MSG_IMPORT_EMPTY, MSG_IMPORT_TOO_MANY};
use crate::AppState;

const MSG_IMPORT_NON_TEXT: &str = "this export contains an item that is not text";

pub(super) fn export(state: &AppState, id: u64, limit: u32) -> Response {
    match copypaste_core::transfer::export(&state.store, &state.keyring, limit) {
        Ok(data) => Response::ok(id, ResponseData::Export(data)),
        Err(copypaste_core::transfer::ExportError::ContentTooLarge) => Response::err(
            id,
            ErrorCode::ContentTooLarge,
            super::messages::MSG_CONTENT_TOO_LARGE,
        ),
        Err(copypaste_core::transfer::ExportError::Store(e)) => storage_error(id, "export", &e),
    }
}

pub(super) fn import(state: &AppState, id: u64, items: Vec<ExportItem>) -> Response {
    let settings = state.settings.get().clone();
    match copypaste_core::transfer::import_with_current_retention_outcome(
        &state.store,
        &state.keyring,
        &settings,
        || state.settings.get().clone(),
        items,
    ) {
        Ok(outcome) => {
            if let Some(oldest) = outcome.oldest_changed_at {
                announce_import(state, oldest);
            }
            Response::ok(id, ResponseData::Import(outcome.data))
        }
        Err(copypaste_core::ImportError::Empty) => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_EMPTY)
        }
        Err(copypaste_core::ImportError::TooMany) => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_TOO_MANY)
        }
        Err(copypaste_core::ImportError::UnsupportedContentType) => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_IMPORT_NON_TEXT)
        }
        Err(copypaste_core::ImportError::Partial {
            oldest_inserted_at,
            source,
            ..
        }) => {
            announce_import(state, oldest_inserted_at);
            match *source {
                copypaste_core::ImportError::Crypto(e) => {
                    warn!(error = ?e, "import partially completed before encryption failed");
                    Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
                }
                copypaste_core::ImportError::Storage(e) => storage_error(id, "import", &e),
                _ => unreachable!("only fatal import errors can be partial"),
            }
        }
        Err(e @ copypaste_core::ImportError::Crypto(_)) => {
            warn!(error = ?e, "import failed to encrypt an item");
            Response::err(id, ErrorCode::Internal, MSG_ENCRYPT)
        }
        Err(copypaste_core::ImportError::Storage(e)) => storage_error(id, "import", &e),
    }
}

/// Announces durable imported rows even when a later row made the request fail.
///
/// Imported capture stamps usually predate an existing upload floor. Reset it
/// to the oldest committed row before waking either transport, or those rows
/// remain stored locally but are never offered to sync.
fn announce_import(state: &AppState, oldest_changed_at: i64) {
    state.modules.note_version(oldest_changed_at);
    state.p2p.node().note_local_version(oldest_changed_at);
    state.note_local_change();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_core::{storage::open_validated, MAX_IMPORT_ITEMS};
    use copypaste_ipc::{ExportData, Method};

    fn seed_legacy_text(state: &AppState, id: &str, content: &str) {
        let key = state.keyring.item_key();
        let (nonce, content_ciphertext) = copypaste_core::encrypt(content.as_bytes(), &key, id)
            .expect("the current item key seals the direct legacy row");
        state
            .store
            .insert(copypaste_core::NewItem {
                id: id.to_string(),
                content_ciphertext,
                nonce,
                content_type: copypaste_ipc::content_type::TEXT.to_string(),
                content_hash: copypaste_core::compute_content_hash(content.as_bytes()),
                search_text: Some(content.to_string()),
                created_at: copypaste_core::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .expect("a pre-limit row is still a valid stored row");
    }

    fn export_of(state: &AppState) -> ExportData {
        match export(state, 1, 0).data {
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
        let exported = export_of(&source);
        assert_eq!(exported.items.len(), 2);

        let (target, _b) = test_state("beta");
        match import_of(&target, exported.items).data {
            Some(ResponseData::Import(result)) => {
                assert_eq!(result.inserted, 2);
                assert_eq!(result.skipped, 0);
            }
            _ => panic!("expected import response"),
        }
        let mut round_tripped = crate::testutil::contents(&target);
        round_tripped.sort();
        assert_eq!(round_tripped, ["first", "second"]);
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
            .set_state_ms(
                "module:copypaste.supabase:cloud_upload_floor_ms",
                after_a_sync,
            )
            .unwrap();

        let old = 1_700_000_000_000;
        import_of(
            &state,
            vec![
                ExportItem {
                    privacy: Default::default(),
                    content: "older".into(),
                    content_type: "text".into(),
                    created_at: old,
                    pinned: false,
                },
                ExportItem {
                    privacy: Default::default(),
                    content: "newer".into(),
                    content_type: "text".into(),
                    created_at: old + 60_000,
                    pinned: false,
                },
            ],
        );

        assert_eq!(
            state
                .meta
                .state_ms("module:copypaste.supabase:cloud_upload_floor_ms")
                .unwrap(),
            old
        );
        let offered = state.store.versions_since(old, 100).unwrap();
        assert_eq!(offered.len(), 2, "the imported history was not offered");
    }

    #[test]
    fn a_failed_import_announces_the_rows_committed_before_the_failure() {
        let (state, _dir) = test_state("partial-import");
        let floor = copypaste_core::now_ms().saturating_add(60_000);
        state
            .meta
            .set_state_ms("module:copypaste.supabase:cloud_upload_floor_ms", floor)
            .unwrap();
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_second_import BEFORE INSERT ON clipboard_items
             WHEN (SELECT COUNT(*) FROM clipboard_items) >= 1
             BEGIN SELECT RAISE(FAIL, 'injected import failure'); END;",
        )
        .unwrap();
        drop(conn);

        let oldest = 1_700_000_000_000;
        let response = import_of(
            &state,
            vec![
                ExportItem {
                    privacy: Default::default(),
                    content: "committed before failure".into(),
                    content_type: "text".into(),
                    created_at: oldest,
                    pinned: false,
                },
                ExportItem {
                    privacy: Default::default(),
                    content: "rejected second item".into(),
                    content_type: "text".into(),
                    created_at: oldest + 60_000,
                    pinned: false,
                },
            ],
        );

        assert!(!response.ok, "the caller must still see the import failure");
        assert_eq!(response.error_code, Some(ErrorCode::Internal));
        assert_eq!(state.store.count().unwrap(), 1);
        assert_eq!(
            state
                .meta
                .state_ms("module:copypaste.supabase:cloud_upload_floor_ms")
                .unwrap(),
            oldest,
            "the committed item must be offered to cloud sync",
        );
        assert_eq!(state.store.versions_since(oldest, 100).unwrap().len(), 1);
    }

    /// An import that inserted nothing must not drag the floor back and make
    /// the next round re-offer the whole history for no reason.
    #[test]
    fn an_import_of_items_already_present_leaves_the_floor_where_it_was() {
        let (state, _dir) = test_state("alpha");
        crate::testutil::add(&state, "already here");
        let exported = export_of(&state);

        let floor = copypaste_core::now_ms();
        state
            .meta
            .set_state_ms("module:copypaste.supabase:cloud_upload_floor_ms", floor)
            .unwrap();
        match import_of(&state, exported.items).data {
            Some(ResponseData::Import(result)) => assert_eq!(result.inserted, 0),
            _ => panic!("expected import response"),
        }

        assert_eq!(
            state
                .meta
                .state_ms("module:copypaste.supabase:cloud_upload_floor_ms")
                .unwrap(),
            floor
        );
    }

    #[test]
    fn duplicate_privacy_and_pin_changes_publish_while_replays_stay_quiet() {
        let (state, _dir) = test_state("duplicate-import");
        let row = copypaste_core::ingest_into(
            &state.store,
            &state.keyring,
            "synthetic secret",
            "text",
            1000,
            &Default::default(),
        )
        .unwrap()
        .into_item();
        let mut item = export_of(&state).items.remove(0);
        item.privacy.secret = true;
        let floor = copypaste_core::now_ms();
        let key = "module:copypaste.supabase:cloud_upload_floor_ms";
        state.meta.set_state_ms(key, floor).unwrap();
        let mut events = state.subscribe();
        let response = import_of(&state, vec![item.clone()]);
        let Some(ResponseData::Import(result)) = response.data else {
            panic!("import response expected")
        };
        assert_eq!((result.inserted, result.skipped_duplicate), (0, 1));
        let secret = state.store.get(&row.id).unwrap().unwrap();
        assert!(secret.clipboard_privacy().secret);
        assert_eq!(state.meta.state_ms(key).unwrap(), secret.created_at);
        assert_eq!(
            events.try_recv().unwrap().event,
            copypaste_ipc::EventKind::Items
        );

        state.meta.set_state_ms(key, floor).unwrap();
        assert!(import_of(&state, vec![item.clone()]).ok);
        assert!(events.try_recv().is_err());
        assert_eq!(state.meta.state_ms(key).unwrap(), floor);

        item.pinned = true;
        assert!(import_of(&state, vec![item]).ok);
        assert!(state.store.get(&row.id).unwrap().unwrap().pinned);
        assert_eq!(
            events.try_recv().unwrap().event,
            copypaste_ipc::EventKind::Items
        );
        assert_eq!(state.meta.state_ms(key).unwrap(), secret.created_at);
    }

    #[test]
    fn duplicate_privacy_change_is_announced_before_a_later_import_failure() {
        let (state, _dir) = test_state("partial-duplicate-import");
        let row = copypaste_core::ingest_into(
            &state.store,
            &state.keyring,
            "synthetic secret",
            "text",
            1000,
            &Default::default(),
        )
        .unwrap()
        .into_item();
        let mut duplicate = export_of(&state).items.remove(0);
        duplicate.privacy.secret = true;
        let mut later = duplicate.clone();
        later.content = "rejected later item".into();
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute_batch("CREATE TRIGGER reject_later_import BEFORE INSERT ON clipboard_items BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
        let key = "module:copypaste.supabase:cloud_upload_floor_ms";
        state
            .meta
            .set_state_ms(key, copypaste_core::now_ms())
            .unwrap();
        let mut events = state.subscribe();
        assert!(!import_of(&state, vec![duplicate, later]).ok);
        let changed = state.store.get(&row.id).unwrap().unwrap();
        assert!(changed.clipboard_privacy().secret);
        assert_eq!(state.meta.state_ms(key).unwrap(), changed.created_at);
        assert_eq!(
            events.try_recv().unwrap().event,
            copypaste_ipc::EventKind::Items
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
            privacy: Default::default(),
            content: "x".into(),
            content_type: "text".into(),
            created_at: 1,
            pinned: false,
        };
        let response = import_of(&state, vec![item; MAX_IMPORT_ITEMS + 1]);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(state.store.count().unwrap(), 0);
    }

    #[test]
    fn a_non_text_import_is_an_invalid_request_and_writes_nothing() {
        let (state, _dir) = test_state("non-text-import");
        let response = import_of(
            &state,
            vec![ExportItem {
                privacy: Default::default(),
                content: "plain content".into(),
                content_type: copypaste_ipc::content_type::IMAGE_PNG.into(),
                created_at: 1,
                pinned: false,
            }],
        );
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(response.error.as_deref(), Some(MSG_IMPORT_NON_TEXT));
        assert_eq!(state.store.count().unwrap(), 0);
    }

    #[test]
    fn legacy_text_export_is_a_typed_non_retryable_refusal() {
        let (state, _dir) = test_state("legacy-export");
        seed_legacy_text(
            &state,
            "daemon-export-legacy",
            &"\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1),
        );

        let response = export(&state, 1, 0);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::ContentTooLarge));
        assert!(response.data.is_none());
        assert!(!ErrorCode::ContentTooLarge.retryable());
    }

    #[test]
    fn aggregate_export_overflow_is_a_typed_non_retryable_refusal() {
        let (state, _dir) = test_state("aggregate-export");
        let first = "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES);
        let second = format!(
            "\u{2}{}",
            "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES - 1)
        );
        seed_legacy_text(&state, "daemon-aggregate-first", &first);
        state
            .store
            .set_pinned("daemon-aggregate-first", true)
            .unwrap();
        seed_legacy_text(&state, "daemon-aggregate-second", &second);

        let response = export(&state, 1, 0);
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::ContentTooLarge));
        assert_eq!(
            response.error.as_deref(),
            Some("This content is too large for this operation. Your history is unchanged.")
        );
        assert!(response.data.is_none());
        assert!(!ErrorCode::ContentTooLarge.retryable());
    }
}
