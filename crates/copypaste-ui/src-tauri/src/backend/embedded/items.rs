//! Embedded history item reads, capture ingest, clipboard writes, and mutations.
//!
//! The functions here adapt the shared store/ingest implementations to the
//! product `Backend` contract. They own publication and version bookkeeping so
//! every successful item mutation has the same observable side effects.

use copypaste_core::{
    ingest, ingest_into_with_capture_source, IngestError, Ingested, ItemCursor, StoredItem,
};
use copypaste_ipc::{ImagePreview, Item};

use super::messages::{MSG_BAD_CURSOR, MSG_EMPTY, MSG_NOT_STORED, MSG_NO_ITEM, MSG_TOO_LARGE};
use super::rows::{clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};
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
                item.truncated = copypaste_ipc::limits::bound_preview(&mut item.content);
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
            Ok(inner.to_wire_page(rows))
        })
        .await
}

pub(super) async fn add(backend: &EmbeddedBackend, content: &str) -> Result<Item> {
    let content = content.to_string();
    backend
        .blocking(move |inner| {
            let settings = inner.settings();
            match ingest(
                &inner.state.store,
                &inner.state.detector,
                &inner.state.keyring,
                &content,
                copypaste_ipc::content_type::TEXT,
                &settings,
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
            match ingest_into_with_capture_source(
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
            let item = inner.fetch(&id)?;
            // Clipboard errors are static by contract, so no caller-supplied
            // path or content can be interpolated into the user-facing error.
            inner
                .clipboard
                .set_text(&item.content)
                .map_err(|message| BackendError::Internal(message.to_string()))?;
            Ok(item)
        })
        .await
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
            inner.fetch(&id)
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
