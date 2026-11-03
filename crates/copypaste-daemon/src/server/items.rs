//! History dispatch through the daemon host: the shared runtime answering
//! with this daemon's clipboard, origin metadata and capture state.

#[allow(unused_imports)]
use crate::testutil::calls::*;

mod tests {
    use super::*;
    use crate::server::dispatch::dispatch_store;
    use crate::testutil::{test_state, test_state_with_clipboard};
    use base64::engine::general_purpose::STANDARD;
    use copypaste_ipc::limits::LIST_PREVIEW_BYTES;
    use copypaste_ipc::{content_type::TEXT, Method};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct UnavailableClipboard;

    impl crate::clipboard::ClipboardSource for UnavailableClipboard {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            None
        }

        fn poll_with_policy(
            &mut self,
            _: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            None
        }

        fn is_available(&self) -> bool {
            false
        }

        fn set_contents(&mut self, _: &str, _: bool) -> anyhow::Result<()> {
            Ok(())
        }

        fn backend_name(&self) -> &'static str {
            "unavailable-test"
        }
    }

    #[test]
    fn status_reports_clipboard_transport_availability() {
        let (available, _dir) = test_state("status-clipboard-available");
        let (unavailable, _dir) = test_state_with_clipboard(
            "status-clipboard-unavailable",
            Box::new(UnavailableClipboard),
        );

        let response_available = status(&available, 1);
        let response_unavailable = status(&unavailable, 2);
        let Some(ResponseData::Status(available)) = response_available.data else {
            panic!("status response expected");
        };
        let Some(ResponseData::Status(unavailable)) = response_unavailable.data else {
            panic!("status response expected");
        };

        assert!(available.clipboard_available);
        assert!(!unavailable.clipboard_available);
    }

    struct ChangingClipboard(Arc<AtomicBool>);

    impl crate::clipboard::ClipboardSource for ChangingClipboard {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            None
        }

        fn poll_with_policy(
            &mut self,
            _: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            None
        }

        fn is_available(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }

        fn set_contents(&mut self, _: &str, _: bool) -> anyhow::Result<()> {
            Ok(())
        }

        fn backend_name(&self) -> &'static str {
            if self.0.load(Ordering::Acquire) {
                "linux-wayland-data-control"
            } else {
                "linux-wayland-data-control-inactive"
            }
        }
    }

    #[test]
    fn status_reads_current_backend_after_reconnect() {
        let connected = Arc::new(AtomicBool::new(false));
        let (state, _dir) = test_state_with_clipboard(
            "status-backend-reconnect",
            Box::new(ChangingClipboard(Arc::clone(&connected))),
        );
        let Some(ResponseData::Status(before)) = status(&state, 1).data else {
            panic!("status response expected");
        };
        connected.store(true, Ordering::Release);
        let Some(ResponseData::Status(after)) = status(&state, 2).data else {
            panic!("status response expected");
        };
        assert!(!before.clipboard_available);
        assert_eq!(
            before.clipboard_backend,
            "linux-wayland-data-control-inactive"
        );
        assert!(after.clipboard_available);
        assert_eq!(after.clipboard_backend, "linux-wayland-data-control");
    }

    #[test]
    fn source_exclusions_report_capture_blocked_without_attribution() {
        let mut capabilities = BTreeMap::from([
            (
                PlatformCapability::ClipboardCapture,
                CapabilityStatus {
                    state: CapabilityState::Available,
                    backend: Some("linux-wayland-data-control".into()),
                    reason: None,
                    permissions: Vec::new(),
                    guarantee: CapabilityGuarantee::Verified,
                    mime_types: Vec::new(),
                    max_bytes: None,
                },
            ),
            (
                PlatformCapability::SourceAttribution,
                CapabilityStatus {
                    state: CapabilityState::Unsupported,
                    backend: Some("linux-wayland-data-control".into()),
                    reason: None,
                    permissions: Vec::new(),
                    guarantee: CapabilityGuarantee::None,
                    mime_types: Vec::new(),
                    max_bytes: None,
                },
            ),
        ]);
        let settings = copypaste_ipc::ConfigData {
            excluded_app_bundle_ids: vec!["org.example.Private".into()],
            ..Default::default()
        };
        apply_capture_policy_capabilities(&mut capabilities, &settings);
        let capture = &capabilities[&PlatformCapability::ClipboardCapture];
        assert_eq!(capture.state, CapabilityState::NeedsCompanion);
        assert_eq!(capture.guarantee, CapabilityGuarantee::None);
        assert_eq!(
            capture.backend.as_deref(),
            Some("linux-wayland-data-control")
        );
    }

    #[test]
    fn explicit_file_import_keeps_each_named_record_and_original_bytes() {
        let (state, dir) = test_state("explicit-file-import");
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"%PDF original").unwrap();
        for name in ["a.pdf", "b.pdf"] {
            let response = dispatch_store(
                &state,
                1,
                Method::ImportFile {
                    path: source.to_string_lossy().into_owned(),
                    filename: name.to_owned(),
                    mime_type: "application/pdf".to_owned(),
                    source_reference: None,
                },
            );
            assert!(response.ok, "file import failed");
        }
        assert_eq!(state.store.count().unwrap(), 2);
        let Some(ResponseData::Page(page)) = list(&state, 2, 10, None).data else {
            panic!("expected page");
        };
        let mut names = page
            .items
            .iter()
            .map(|item| {
                item.file_details
                    .as_ref()
                    .unwrap()
                    .filename
                    .clone()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["a.pdf", "b.pdf"]);
        for item in page.items {
            let saved = dir.path().join(format!("{}.saved", item.id));
            assert!(save_file(&state, 3, &item.id, &saved.to_string_lossy()).ok);
            assert_eq!(std::fs::read(saved).unwrap(), b"%PDF original");
        }
    }

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
                search_text: Some(content.to_string()),
                created_at: copypaste_core::now_ms(),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
            })
            .expect("a pre-limit row is still a valid stored row");
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
            None,
            metadata,
            &state.settings.get(),
        )
        .unwrap()
        .into_item()
    }

    #[test]
    fn text_items_carry_their_semantic_kind_and_color_swatch() {
        let (state, _dir) = test_state("semantic-wire-item");
        let item = match add(&state, 1, "#aabbcc").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };

        assert_eq!(item.semantic_kind, Some(copypaste_ipc::SemanticKind::Color));
        assert_eq!(item.color_rgba, Some(0xaabb_ccff));
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
        assert!(whole
            .items
            .iter()
            .all(|item| item.content.capacity() <= LIST_PREVIEW_BYTES));
        for item in &mut whole.items {
            item.content = item.content.repeat(BODY / LIST_PREVIEW_BYTES);
            item.truncated = false;
        }
        let unbounded = serde_json::to_string(&Response::ok(1, ResponseData::Page(whole)))
            .unwrap()
            .len();

        let item_bound = LIST_PREVIEW_BYTES * 2 + 768;
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
    fn a_renamed_device_identity_survives_reopening() {
        let (state, _dir) = test_state("old name");
        assert!(set_device_name(&state, 1, "  Kitchen Mac  ").ok);

        let reopened = copypaste_runtime::Meta::open(&state.store, "ignored hostname").unwrap();
        assert_eq!(reopened.device_id(), state.meta.device_id());
        assert_eq!(reopened.device_name(), "Kitchen Mac");
        match status(&state, 2).data {
            Some(ResponseData::Status(status)) => {
                assert_eq!(status.device_name, "Kitchen Mac");
                assert_eq!(status.device_id.as_deref(), Some(state.meta.device_id()));
            }
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
    fn search_stops_at_the_ciphertext_budget_before_building_a_page() {
        let (state, _dir) = test_state("search-byte-budget");
        seed_legacy_text(
            &state,
            "search-oversize",
            &format!(
                "bounded search bounded search {}",
                "x".repeat(MAX_PAGE_CONTENT_BYTES)
            ),
            TEXT,
        );
        seed_legacy_text(&state, "search-follower", "bounded search", TEXT);
        let ranked = state.store.search("bounded search", 20).unwrap();

        let page = match search(&state, 1, "bounded search", 20).data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("search must return a page, got {other:?}"),
        };
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, ranked[0].id);
        assert!(page.next_cursor.is_none());
        assert!(serde_json::to_string(&page).unwrap().len() <= copypaste_ipc::MAX_FRAME_BYTES);
    }

    #[test]
    fn spoiler_conceal_of_an_empty_session_does_not_reload_history() {
        let (state, _dir, writes) =
            crate::testutil::test_state_watching_clipboard("spoiler-idempotent");
        let mut events = state.subscribe_events();
        let response = set_spoiler_revealed(&state, 1, None, false);
        assert!(response.ok);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(writes.count(), 0);
    }

    #[test]
    fn spoiler_visibility_notifies_both_windows_without_copy_or_persistence() {
        let (state, _dir, writes) =
            crate::testutil::test_state_watching_clipboard("spoiler-visibility");
        let item = match add(&state, 1, "hidden body").data {
            Some(ResponseData::Item(item)) => item,
            _ => panic!("expected item"),
        };
        assert!(set_secret(&state, 2, &item.id, true).ok);
        let mut first = state.subscribe_events();
        let mut second = state.subscribe_events();
        let revealed = set_spoiler_revealed(&state, 3, Some(&item.id), true);
        assert!(
            matches!(revealed.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids == [item.id.clone()])
        );
        for receiver in [&mut first, &mut second] {
            let event = receiver.try_recv().unwrap();
            assert_eq!(event.event, copypaste_ipc::EventKind::Items);
            assert!(!event.captured);
        }
        let snapshot = super::super::dispatch::dispatch_store(
            &state,
            4,
            copypaste_ipc::Method::GetSpoilerState,
        );
        assert!(
            matches!(snapshot.data, Some(ResponseData::SpoilerState { revealed_ids, .. }) if revealed_ids == [item.id.clone()])
        );
        assert!(set_secret(&state, 5, &item.id, true).ok);
        assert!(state.spoilers.snapshot().1.is_empty());
        assert!(set_spoiler_revealed(&state, 6, Some(&item.id), true).ok);
        assert!(set_spoiler_revealed(&state, 7, None, false).ok);
        assert!(state.spoilers.snapshot().1.is_empty());
        assert!(!set_spoiler_revealed(&state, 8, Some("missing"), true).ok);
        assert_eq!(writes.count(), 0);
    }

    #[test]
    fn secret_choice_returns_updated_content_without_writing_the_clipboard() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("secret-choice");
        let item = match add(&state, 1, "toggle text").data {
            Some(ResponseData::Item(item)) => item,
            _ => panic!("expected item"),
        };
        for secret in [true, false] {
            let response = set_secret(&state, 2, &item.id, secret);
            assert!(response.ok, "secret update failed");
            let Some(ResponseData::Item(updated)) = response.data else {
                panic!("expected item");
            };
            assert_eq!(updated.privacy.secret, secret);
            assert_eq!(updated.content, "toggle text");
        }
        assert_eq!(writes.count(), 0);
        assert_eq!(
            set_secret(&state, 3, "missing", true).error_code,
            Some(ErrorCode::NotFound)
        );
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
        assert_eq!(
            crate::notify::count_requests(|| {
                assert!(copy(&state, 3, &added.id).ok);
            }),
            1
        );
        assert_eq!(writes.count(), 1);

        assert_eq!(
            crate::notify::count_requests(|| {
                assert!(copy_plain_text(&state, 4, &added.id).ok);
            }),
            1
        );
        assert_eq!(
            writes.count(),
            2,
            "plain-text copy did not reach the clipboard"
        );

        // An unknown id is not_found, not an empty success.
        let missing = get(&state, 5, "00000000-0000-0000-0000-000000000000");
        assert_eq!(missing.error_code, Some(ErrorCode::NotFound));
        assert_eq!(
            crate::notify::count_requests(|| {
                assert!(!copy(&state, 6, "missing").ok);
            }),
            0
        );
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
                _ => panic!("expected an item"),
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
        assert!(listed[0].image_details.is_none());

        let detail = match get(&state, 2, &image.id).data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        let metadata = detail.image_details.expect("selected image metadata");
        assert_eq!((metadata.width, metadata.height), (1, 1));
        assert_eq!(metadata.size_bytes, source.len() as u64);

        let preview = match image_preview(&state, 3, &image.id, None, None).data {
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
    fn native_copy_uses_a_file_source_path_while_plain_text_copy_refuses_binary_items() {
        use crate::testutil::WrittenPayload;

        let (state, dir, writes) =
            crate::testutil::test_state_watching_clipboard("native-copy-payloads");
        let image_bytes = b"native image bytes";
        let image = binary_item(
            &state,
            copypaste_ipc::content_type::IMAGE_PNG,
            image_bytes,
            None,
        );
        let source = dir.path().join("report.bin");
        std::fs::write(&source, b"native file bytes").unwrap();
        let metadata = copypaste_core::FileMetadata::with_source_reference(
            "report.bin",
            "application/octet-stream",
            source.to_str().unwrap(),
        )
        .unwrap();
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
                WrittenPayload::Text(source.to_string_lossy().into_owned()),
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
        for expected in ["[image]", source.to_str().unwrap(), "[unsupported]"] {
            assert!(
                listed.iter().any(|item| item.content == expected),
                "{expected}"
            );
        }
    }

    #[test]
    fn copying_ocr_text_promotes_the_image_without_replacing_its_payload() {
        let (state, _dir, writes) = crate::testutil::test_state_watching_clipboard("ocr-copy");
        let image = binary_item(
            &state,
            copypaste_ipc::content_type::IMAGE_PNG,
            b"image bytes",
            None,
        );
        let original = state.store.get(&image.id).unwrap().unwrap();
        assert!(add(&state, 1, "newer text").ok);
        assert_eq!(
            crate::notify::count_requests(|| {
                assert!(copy_text(&state, 2, &image.id, "recognized text").ok);
            }),
            1
        );
        assert_eq!(state.store.list(10, 0).unwrap()[0].id, image.id);
        assert_eq!(
            state
                .store
                .get(&image.id)
                .unwrap()
                .unwrap()
                .content_ciphertext,
            original.content_ciphertext
        );
        assert_eq!(
            writes.entries(),
            [crate::testutil::WrittenPayload::Text(
                "recognized text".into()
            )]
        );
        assert_eq!(state.store.count().unwrap(), 2);
    }

    #[test]
    fn remote_file_copy_uses_authenticated_bytes_instead_of_foreign_path() {
        use crate::testutil::WrittenPayload;

        let (state, _dir, writes) =
            crate::testutil::test_state_watching_clipboard("remote-file-copy");
        let metadata = copypaste_core::FileMetadata::with_source_reference(
            "report.bin",
            "application/octet-stream",
            "/another-device/report.bin",
        )
        .unwrap();
        let local = binary_item(
            &state,
            copypaste_ipc::content_type::FILE,
            b"remote bytes",
            Some(&metadata),
        );
        crate::testutil::calls::store_source(&state)
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "remote-file-copy",
                content: "",
                binary_content: Some(b"remote bytes"),
                payload_metadata: local.payload_metadata.as_deref(),
                content_type: copypaste_ipc::content_type::FILE,
                created_at: copypaste_core::now_ms() + 1,
                deleted: false,
                content_hash: None,
                origin_device_id: "another-device",
                app_bundle_id: None,
                app_name: None,
            })
            .unwrap();

        assert!(copy(&state, 1, "remote-file-copy").ok);
        assert!(writes.entries().iter().all(|entry| matches!(
            entry,
            WrittenPayload::File { bytes, metadata }
                if bytes == b"remote bytes"
                    && metadata.as_ref().is_some_and(|file| file.source_reference.is_none())
        )));
    }

    #[test]
    fn image_preview_bounds_leave_the_stored_and_copied_original_unchanged() {
        let (state, _dir, writes) =
            crate::testutil::test_state_watching_clipboard("bounded-preview-original");
        let mut source = Vec::new();
        image::DynamicImage::new_rgba8(1200, 1200)
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        let item = binary_item(
            &state,
            copypaste_ipc::content_type::IMAGE_PNG,
            &source,
            None,
        );
        let original = state.store.get(&item.id).unwrap().unwrap();
        let preview = match image_preview(
            &state,
            1,
            &item.id,
            None,
            Some(copypaste_ipc::ImagePreviewBounds {
                width: 1400,
                height: 320,
            }),
        )
        .data
        {
            Some(ResponseData::ImagePreview(preview)) => preview,
            other => panic!("{other:?}"),
        };
        assert_eq!((preview.width, preview.height), (320, 320));
        let stored = state.store.get(&item.id).unwrap().unwrap();
        assert_eq!(stored.content_ciphertext, original.content_ciphertext);
        assert_eq!(
            copypaste_core::open_binary(
                &stored.content_ciphertext,
                &state.keyring.item_key(),
                &stored.id,
            )
            .unwrap()
            .as_slice(),
            source.as_slice(),
        );
        assert!(copy(&state, 2, &item.id).ok);
        assert_eq!(
            writes.entries(),
            vec![crate::testutil::WrittenPayload::Image(source)]
        );
    }

    #[test]
    fn save_file_writes_authenticated_bytes_to_a_new_destination() {
        let (state, dir) = test_state("save-file-payload");
        let metadata = copypaste_core::FileMetadata::with_source_reference(
            "report.bin",
            "application/octet-stream",
            "/Users/person/Documents/report.bin",
        )
        .unwrap();
        let stored = binary_item(
            &state,
            copypaste_ipc::content_type::FILE,
            b"downloaded bytes",
            Some(&metadata),
        );
        let destination = dir.path().join("downloaded-report.bin");

        assert!(save_file(&state, 1, &stored.id, &destination.to_string_lossy()).ok);
        assert_eq!(std::fs::read(&destination).unwrap(), b"downloaded bytes");
        assert_eq!(
            save_file(&state, 2, &stored.id, &destination.to_string_lossy()).error_code,
            Some(ErrorCode::InvalidRequest)
        );
    }

    #[test]
    fn save_file_exports_original_image_for_ocr_without_overwriting() {
        let (state, dir) = test_state("save-image-for-ocr");
        let original = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=")
            .unwrap();
        let stored = binary_item(
            &state,
            copypaste_ipc::content_type::IMAGE_PNG,
            &original,
            None,
        );
        let destination = dir.path().join("ocr-input.png");

        assert!(save_file(&state, 1, &stored.id, &destination.to_string_lossy()).ok);
        assert_eq!(std::fs::read(&destination).unwrap(), original);
        assert_eq!(
            save_file(&state, 2, &stored.id, &destination.to_string_lossy()).error_code,
            Some(ErrorCode::InvalidRequest)
        );
        assert_eq!(std::fs::read(&destination).unwrap(), original);
        assert_eq!(state.store.count().unwrap(), 1);

        let ResponseData::Item(text) = add(&state, 3, "text").data.unwrap() else {
            panic!("text clip must be added");
        };
        let text_destination = dir.path().join("text.png");
        assert_eq!(
            save_file(&state, 4, &text.id, &text_destination.to_string_lossy()).error_code,
            Some(ErrorCode::UnsupportedContent)
        );
        assert!(!text_destination.exists());
    }

    #[test]
    fn image_preview_refuses_an_undecodable_image() {
        let (state, _dir) = test_state("image-preview-invalid");
        let image = copypaste_core::ingest_binary_into_with_capture_context(
            &state.store,
            &state.keyring,
            b"bytes",
            copypaste_ipc::content_type::IMAGE_PNG,
            copypaste_core::now_ms(),
            None,
            None,
            &state.settings.get(),
        )
        .unwrap()
        .into_item();

        let response = image_preview(&state, 1, &image.id, None, None);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(response.error.as_deref(), Some(MSG_IMAGE_PREVIEW));
    }

    #[test]
    fn image_preview_refuses_an_unknown_content_class() {
        let (state, _dir) = test_state("image-preview-unknown");
        let unknown = binary_item(&state, "application/x-future", b"future bytes", None);

        let response = image_preview(&state, 1, &unknown.id, None, None);
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
        crate::testutil::calls::store_source(&state)
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

    #[test]
    fn history_query_uses_the_local_facet_identity_for_local_captures() {
        let (state, _dir) = test_state("history-device-filter");
        let local = match add(&state, 1, "captured here").data {
            Some(ResponseData::Item(item)) => item,
            other => panic!("{other:?}"),
        };
        crate::testutil::calls::store_source(&state)
            .apply_version(&copypaste_core::RemoteVersion {
                item_id: "from-the-phone",
                content: "captured remotely",
                binary_content: None,
                payload_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT,
                created_at: copypaste_core::now_ms(),
                deleted: false,
                content_hash: None,
                origin_device_id: "remote-device",
                app_bundle_id: None,
                app_name: None,
            })
            .expect("the remote row is stored");

        let local_facet = match history_facets(&state, 2).data {
            Some(ResponseData::HistoryFacets(facets)) => facets
                .origin_devices
                .into_iter()
                .find(|facet| facet.id == state.meta.device_id())
                .expect("the local device facet"),
            other => panic!("{other:?}"),
        };
        let response = history_query(
            &state,
            3,
            &HistoryQuery {
                origin_device_id: Some(local_facet.id),
                ..HistoryQuery::default()
            },
            10,
            None,
        );
        let page = match response.data {
            Some(ResponseData::Page(page)) => page,
            other => panic!("{other:?}"),
        };
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, local.id);
        assert_eq!(page.items[0].origin_device_id, state.meta.device_id());
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
            .store
            .set_state_all(&[
                (
                    "module:copypaste.supabase:cloud_upload_floor_ms",
                    &ahead.to_string(),
                ),
                (
                    "module:copypaste.supabase:cloud_upload_floor_item_id",
                    "zzzz",
                ),
            ])
            .unwrap();

        assert!(delete(&state, 2, &added.id).ok);

        let floor = state
            .store
            .state_ms("module:copypaste.supabase:cloud_upload_floor_ms")
            .unwrap();
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
            .store
            .set_state_ms("module:copypaste.supabase:cloud_upload_floor_ms", ahead)
            .unwrap();

        assert!(delete_all(&state, 2, None).ok);

        let floor = state
            .store
            .state_ms("module:copypaste.supabase:cloud_upload_floor_ms")
            .unwrap();
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
