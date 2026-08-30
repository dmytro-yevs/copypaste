//! Embedded history item reads, capture ingest, clipboard writes, and mutations.
//!
//! The functions here adapt the shared store/ingest implementations to the
//! product `Backend` contract. They own publication and version bookkeeping so
//! every successful item mutation has the same observable side effects.

use copypaste_core::{IngestError, Ingested, ItemCursor, StoredItem};
use copypaste_ipc::{ImagePreview, Item};

use super::messages::{
    MSG_BAD_CURSOR, MSG_EMPTY, MSG_NOT_STORED, MSG_NO_ITEM, MSG_TOO_LARGE, MSG_UNSUPPORTED_CONTENT,
};
use super::rows::{bound_item_preview, clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};
use super::EmbeddedBackend;
use crate::backend::{BackendError, CaptureWrite, Page, Result};
use crate::capture::model::CaptureSource;

pub(super) async fn list(
    backend: &EmbeddedBackend,
    limit: u32,
    cursor: Option<&str>,
) -> Result<Page> {
    let limit = clamp_page(limit, DEFAULT_LIST_PAGE);
    let after = match cursor.map(ItemCursor::parse).transpose() {
        Ok(after) => after,
        // Restarting at the head would replay history during load-more without
        // giving the caller any way to distinguish it from a real first page.
        Err(_) => return Err(BackendError::Invalid(MSG_BAD_CURSOR)),
    };
    backend
        .blocking(move |inner| {
            let page = inner
                .state
                .store
                .list_from(after.as_ref(), limit)
                .map_err(|_| BackendError::internal("history could not be read"))?;
            let next = page.next.map(|cursor| cursor.token());
            let mut wire = inner.to_wire_page(page.items);
            for item in &mut wire.items {
                bound_item_preview(item);
            }
            // The cursor belongs to the store page, not the rows that survived
            // decryption; unreadable rows still occupy their original window.
            wire.next_cursor = next;
            Ok(wire)
        })
        .await
}

pub(super) async fn search(backend: &EmbeddedBackend, query: &str, limit: u32) -> Result<Page> {
    let limit = clamp_page(limit, DEFAULT_SEARCH_PAGE);
    let query = query.to_string();
    backend
        .blocking(move |inner| {
            let rows = inner
                .state
                .store
                .search(&query, limit)
                .map_err(|_| BackendError::internal("history could not be searched"))?;
            // This read-time layer protects databases written before sensitive
            // rows were excluded from FTS at write time (storage invariant I7).
            let rows: Vec<StoredItem> = rows.into_iter().filter(|row| !row.is_sensitive).collect();
            let mut page = inner.to_wire_page(rows);
            for item in &mut page.items {
                bound_item_preview(item);
            }
            Ok(page)
        })
        .await
}

pub(super) async fn add(backend: &EmbeddedBackend, content: &str) -> Result<Item> {
    let content = content.to_string();
    backend
        .blocking(move |inner| {
            let settings = inner.settings();
            match copypaste_core::ingest::ingest_with_current_retention(
                &inner.state.store,
                &inner.state.detector,
                &inner.state.keyring,
                &content,
                copypaste_ipc::content_type::TEXT,
                &settings,
                || inner.settings(),
            ) {
                Ok(ingested) => {
                    let item = inner.to_wire(ingested.into_item())?;
                    inner.note_version_written(item.created_at);
                    inner.note_local_version(item.created_at);
                    inner.publish_items(false, 0);
                    Ok(item)
                }
                Err(IngestError::Empty) => Err(BackendError::Invalid(MSG_EMPTY)),
                Err(IngestError::TooLarge) => Err(BackendError::Invalid(MSG_TOO_LARGE)),
                Err(error) => {
                    tracing::warn!(error = ?error, "a capture could not be stored");
                    Err(BackendError::internal(MSG_NOT_STORED))
                }
            }
        })
        .await
}

pub(super) async fn add_captured(
    backend: &EmbeddedBackend,
    content: &str,
    source: CaptureSource,
    app_bundle_id: Option<&str>,
    app_name: Option<&str>,
) -> Result<Option<CaptureWrite>> {
    let content = content.to_string();
    let app_bundle_id = app_bundle_id.map(str::to_owned);
    let app_name = app_name.map(str::to_owned);
    backend
        .blocking(move |inner| {
            let settings = inner.settings();
            if settings.private_mode {
                return Ok(None);
            }
            if source.requires_external_attribution()
                && !settings.excluded_app_bundle_ids.is_empty()
            {
                let allowed = app_bundle_id.as_ref().is_some_and(|id| {
                    !settings
                        .excluded_app_bundle_ids
                        .iter()
                        .any(|excluded| excluded.eq_ignore_ascii_case(id))
                });
                if !allowed {
                    return Ok(None);
                }
            }

            let sensitive_floor = app_bundle_id
                .as_deref()
                .is_some_and(copypaste_core::sensitive::is_password_manager_app);
            match copypaste_core::ingest::ingest_into_with_capture_source_with_current_retention(
                &inner.state.store,
                &inner.state.detector,
                &inner.state.keyring,
                &content,
                copypaste_ipc::content_type::TEXT,
                copypaste_core::now_ms(),
                sensitive_floor,
                app_bundle_id.as_deref(),
                app_name.as_deref(),
                &settings,
                || inner.settings(),
            ) {
                Ok(ingested) => {
                    let (item, saved) = match ingested {
                        Ingested::Stored(item) => (item, true),
                        Ingested::Duplicate(item) => (item, false),
                    };
                    let item = inner.to_wire(item)?;
                    inner.note_version_written(item.created_at);
                    inner.note_local_version(item.created_at);
                    inner.publish_items(true, 0);
                    Ok(Some(CaptureWrite { item, saved }))
                }
                Err(IngestError::Empty) => Err(BackendError::Invalid(MSG_EMPTY)),
                Err(IngestError::TooLarge) => Err(BackendError::Invalid(MSG_TOO_LARGE)),
                Err(error) => {
                    tracing::warn!(?error, "a captured clip could not be stored");
                    Err(BackendError::internal(MSG_NOT_STORED))
                }
            }
        })
        .await
}

pub(super) async fn get(backend: &EmbeddedBackend, id: &str) -> Result<Item> {
    let id = id.to_string();
    backend.blocking(move |inner| inner.fetch(&id)).await
}

pub(super) async fn image_preview(backend: &EmbeddedBackend, id: &str) -> Result<ImagePreview> {
    let id = id.to_string();
    backend
        .blocking(move |inner| inner.image_preview(&id))
        .await
}

pub(super) async fn copy(backend: &EmbeddedBackend, id: &str) -> Result<Item> {
    let id = id.to_string();
    backend
        .blocking(move |inner| {
            let (item, payload) = inner.fetch_with_payload(&id)?;
            inner.refuse_oversized_text(&payload)?;
            inner.clipboard.write(&payload).map_err(clipboard_error)?;
            Ok(item)
        })
        .await
}

pub(super) async fn copy_plain_text(backend: &EmbeddedBackend, id: &str) -> Result<Item> {
    let id = id.to_string();
    backend
        .blocking(move |inner| {
            let (item, payload) = inner.fetch_with_payload(&id)?;
            inner.refuse_oversized_text(&payload)?;
            if payload.plain_text().is_none() {
                return Err(BackendError::UnsupportedContent(MSG_UNSUPPORTED_CONTENT));
            }
            inner.clipboard.write(&payload).map_err(clipboard_error)?;
            Ok(item)
        })
        .await
}

fn clipboard_error(error: copypaste_core::ClipboardWriteError) -> BackendError {
    match error {
        copypaste_core::ClipboardWriteError::UnsupportedContent => {
            BackendError::UnsupportedContent(MSG_UNSUPPORTED_CONTENT)
        }
        copypaste_core::ClipboardWriteError::Failed => {
            BackendError::internal("that item could not be copied to the clipboard")
        }
    }
}

pub(super) async fn delete(backend: &EmbeddedBackend, id: &str) -> Result<()> {
    let id = id.to_string();
    backend
        .blocking(move |inner| {
            let mutation_started = copypaste_core::now_ms();
            // An unknown id is not a successful no-op: callers must know that
            // they deleted nothing, matching the daemon item contract.
            match inner.state.store.delete(&id) {
                Ok(true) => {
                    inner.note_version_written(mutation_started);
                    inner.note_local_version(mutation_started);
                    inner.publish_items(false, 0);
                    Ok(())
                }
                Ok(false) => Err(BackendError::NotFound(MSG_NO_ITEM)),
                Err(_) => Err(BackendError::internal("that item could not be deleted")),
            }
        })
        .await
}

pub(super) async fn set_pinned(backend: &EmbeddedBackend, id: &str, pinned: bool) -> Result<Item> {
    let id = id.to_string();
    backend
        .blocking(move |inner| {
            match inner.state.store.set_pinned(&id, pinned) {
                Ok(true) => {}
                Ok(false) => return Err(BackendError::NotFound(MSG_NO_ITEM)),
                Err(_) => return Err(BackendError::internal("that item could not be changed")),
            }
            inner.note_local_version(copypaste_core::now_ms());
            inner.publish_items(false, 0);
            inner.fetch_preview(&id)
        })
        .await
}

pub(super) async fn reorder_pinned(backend: &EmbeddedBackend, ids: &[String]) -> Result<()> {
    let ids = ids.to_vec();
    backend
        .blocking(move |inner| {
            // The store owns stale/unpinned-id handling. A zero count remains
            // success so a concurrent sync cannot invalidate the gesture.
            let renumbered = inner
                .state
                .store
                .reorder_pinned(&ids)
                .map_err(|_| BackendError::internal("the pinned order could not be changed"))?;
            if renumbered > 0 {
                inner.publish_items(false, 0);
            }
            Ok(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;

    fn binary(
        backend: &EmbeddedBackend,
        content_type: &str,
        bytes: &[u8],
        metadata: Option<&copypaste_core::FileMetadata>,
    ) -> copypaste_core::StoredItem {
        copypaste_core::ingest_binary_into_with_capture_context(
            &backend.inner.state.store,
            &backend.inner.state.keyring,
            bytes,
            content_type,
            copypaste_core::now_ms(),
            false,
            None,
            metadata,
            &backend.inner.settings(),
        )
        .unwrap()
        .into_item()
    }

    fn seed_legacy_text(backend: &EmbeddedBackend, id: &str, content: &str) {
        let key = backend.inner.state.keyring.item_key();
        let (nonce, content_ciphertext) = copypaste_core::encrypt(content.as_bytes(), &key, id)
            .expect("the current item key seals the direct legacy row");
        backend
            .inner
            .state
            .store
            .insert(copypaste_core::NewItem {
                id: id.to_string(),
                content_ciphertext,
                nonce,
                content_type: copypaste_ipc::content_type::TEXT.to_string(),
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

    #[tokio::test]
    async fn text_copy_uses_the_full_authenticated_body_not_the_list_preview() {
        let (backend, clipboard, _dir) = backend();
        let body = "x".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES * 2);
        let item = backend.add(&body).await.unwrap();
        let listed = backend.list(20, None).await.unwrap();
        assert!(listed.items[0].truncated);
        assert_ne!(listed.items[0].content, body);

        backend.copy(&item.id).await.unwrap();
        backend.copy_as_plain_text(&item.id).await.unwrap();
        assert_eq!(clipboard.entries(), vec![body.clone(), body]);
    }

    #[tokio::test]
    async fn pinning_an_ordinary_body_keeps_its_full_response() {
        let (backend, _clipboard, _dir) = backend();
        let body = "x".repeat(copypaste_ipc::limits::LIST_PREVIEW_BYTES * 2);
        let item = backend.add(&body).await.unwrap();

        for pinned in [true, false] {
            let updated = backend.set_pinned(&item.id, pinned).await.unwrap();
            assert_eq!(updated.content, body);
            assert!(!updated.truncated);
        }
    }

    #[tokio::test]
    async fn legacy_text_refuses_full_operations_but_pin_returns_a_preview() {
        let (backend, clipboard, _dir) = backend();
        let body = format!(
            "needle {}",
            "\u{1}".repeat(copypaste_ipc::MAX_CONTENT_BYTES + 1 - "needle ".len())
        );
        seed_legacy_text(&backend, "embedded-legacy", &body);
        let before = backend
            .inner
            .state
            .store
            .get("embedded-legacy")
            .unwrap()
            .unwrap()
            .content_ciphertext;

        for result in [
            backend.get("embedded-legacy").await,
            backend.copy("embedded-legacy").await,
            backend.copy_as_plain_text("embedded-legacy").await,
        ] {
            let error = result.unwrap_err();
            assert!(matches!(error, BackendError::ContentTooLarge(_)));
            assert_eq!(error.ui_error().code, "content_too_large");
            assert!(!error.ui_error().retryable);
        }
        assert!(clipboard.entries().is_empty());

        let listed = backend.list(20, None).await.unwrap();
        assert!(listed.items[0].truncated);
        assert!(listed.items[0].content.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);
        let searched = backend.search("needle", 20).await.unwrap();
        assert!(searched.items[0].truncated);
        assert!(searched.items[0].content.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);

        for pinned in [true, false] {
            let item = backend.set_pinned("embedded-legacy", pinned).await.unwrap();
            assert!(item.truncated);
            assert!(item.content.len() <= copypaste_ipc::limits::LIST_PREVIEW_BYTES);
        }
        assert_eq!(
            backend
                .inner
                .state
                .store
                .get("embedded-legacy")
                .unwrap()
                .unwrap()
                .content_ciphertext,
            before
        );
    }

    #[tokio::test]
    async fn android_refuses_binary_and_unknown_content_without_writing_labels() {
        let (backend, clipboard, _dir) = backend();
        let metadata =
            copypaste_core::FileMetadata::new("report.bin", "application/octet-stream").unwrap();
        let image = binary(
            &backend,
            copypaste_ipc::content_type::IMAGE_PNG,
            b"image bytes",
            None,
        );
        let file = binary(
            &backend,
            copypaste_ipc::content_type::FILE,
            b"file bytes",
            Some(&metadata),
        );
        let unknown = binary(&backend, "application/x-future", b"future bytes", None);

        for row in [&image, &file, &unknown] {
            for error in [
                backend.copy(&row.id).await.unwrap_err(),
                backend.copy_as_plain_text(&row.id).await.unwrap_err(),
            ] {
                assert!(
                    matches!(error, BackendError::UnsupportedContent(_)),
                    "{error:?}"
                );
                assert_eq!(error.ui_error().code, "unsupported_content");
                assert!(!error.ui_error().retryable);
            }
        }
        assert!(clipboard.entries().is_empty());

        let labels: Vec<String> = backend
            .list(20, None)
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|item| item.content)
            .collect();
        for label in ["[image]", "[file]", "[unsupported]"] {
            assert!(labels.iter().any(|content| content == label), "{label}");
        }
    }
}
