//! Stored rows into wire items.
//!
//! Separated from the command surface in `super` so that "what a row becomes"
//! is one small thing to read: the AAD binding and the count of what would not
//! open are the whole file.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_core::{open_binary, origin_or, thumbnail_png, ClipboardPayload, StoredItem};
use copypaste_ipc::{ImagePreview, Item};

use super::open::Inner;
use crate::backend::{BackendError, Page, Result};

pub(super) use copypaste_ipc::{clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};

const MSG_NO_ITEM: &str = "That item is no longer there.";
pub(super) const MSG_CONTENT_TOO_LARGE: &str =
    "This content is too large for this operation. Your history is unchanged.";

pub(super) fn bound_item_preview(item: &mut Item) {
    item.truncated = copypaste_ipc::limits::bound_preview(&mut item.content);
}

impl Inner {
    /// Decrypt one stored row into its wire form.
    ///
    /// Two library calls and a struct literal. The item id is the AAD, so a row
    /// decrypted under another row's identity fails authentication rather than
    /// falling back to a plaintext read (AGENTS.md rule 4, "fail closed").
    pub(super) fn to_wire(&self, row: StoredItem) -> Result<Item> {
        let device_id = origin_or(&row.origin_device_id, &self.state.device_id).to_string();
        let names = self
            .state
            .store
            .device_names(std::slice::from_ref(&device_id))
            .unwrap_or_default();
        self.to_wire_with(row, &names).map(|(item, _)| item)
    }

    /// [`Inner::to_wire`] with the page's device names already resolved.
    fn to_wire_with(
        &self,
        row: StoredItem,
        names: &HashMap<String, String>,
    ) -> Result<(Item, ClipboardPayload)> {
        let key = self.state.keyring.item_key();
        let payload = ClipboardPayload::open(&row, &key)
            .map_err(|_| BackendError::internal("that item could not be decrypted"))?;
        // A row captured here stores no origin; substituting this device's id
        // is what makes the field mean the same thing on both platforms
        // (`copypaste_core::origin_or`). The name is `None` rather than a guess
        // until a session with that device has told us one.
        let device_id = origin_or(&row.origin_device_id, &self.state.device_id).to_string();
        let origin_device_name = names.get(&device_id).cloned();
        let content = payload.display_text();
        let sensitive_finding = (!row.is_sensitive
            && copypaste_ipc::content_type::is_text(&row.content_type))
        .then(|| self.state.detector.inert_finding_metadata(&content))
        .flatten();
        let too_large_to_sync =
            copypaste_cloud::sync::too_large_to_sync(&row.content_type, payload.byte_len());
        let item = Item {
            id: row.id,
            content,
            content_type: row.content_type,
            created_at: row.created_at,
            pinned: row.pinned,
            is_sensitive: row.is_sensitive,
            sensitive_finding,
            origin_device_id: device_id,
            origin_device_name,
            source_app_bundle_id: row.app_bundle_id,
            source_app_name: row.app_name,
            too_large_to_sync,
            truncated: false,
        };
        Ok((item, payload))
    }

    /// Decrypt a page, dropping any row that will not open — and **counting**
    /// what was dropped.
    ///
    /// One unreadable row must not blank a whole page: the other items are
    /// still the user's data (AGENTS.md rule 4). But dropping it silently makes
    /// a short page indistinguishable from a small history, which is parity
    /// finding 17 / `CopyPaste-00zz`. The count is what lets the UI say "3
    /// items could not be read" instead of showing three fewer rows.
    pub(super) fn to_wire_page(&self, rows: Vec<StoredItem>) -> Page {
        // One name query for the page rather than one per row: a page is up to
        // `MAX_PAGE` items and this runs on every list and every search.
        let device_ids: Vec<String> = rows
            .iter()
            .map(|row| origin_or(&row.origin_device_id, &self.state.device_id).to_string())
            .collect();
        let names = self
            .state
            .store
            .device_names(&device_ids)
            .unwrap_or_default();

        let mut page = Page::default();
        for row in rows {
            let id = row.id.clone();
            match self.to_wire_with(row, &names) {
                Ok((item, _)) => page.items.push(item),
                Err(_) => {
                    tracing::warn!(%id, "skipping an item that failed to decrypt");
                    page.skipped_undecryptable = page.skipped_undecryptable.saturating_add(1);
                }
            }
        }
        page
    }

    pub(super) fn fetch(&self, id: &str) -> Result<Item> {
        let (item, payload) = self.fetch_with_payload(id)?;
        self.refuse_oversized_text(&payload)?;
        Ok(item)
    }

    pub(super) fn fetch_preview(&self, id: &str) -> Result<Item> {
        let (mut item, payload) = self.fetch_with_payload(id)?;
        if is_oversized_text(&payload) {
            bound_item_preview(&mut item);
        }
        Ok(item)
    }

    pub(super) fn refuse_oversized_text(&self, payload: &ClipboardPayload) -> Result<()> {
        if is_oversized_text(payload) {
            return Err(BackendError::ContentTooLarge(MSG_CONTENT_TOO_LARGE));
        }
        Ok(())
    }

    pub(super) fn fetch_with_payload(&self, id: &str) -> Result<(Item, ClipboardPayload)> {
        match self.state.store.get(id) {
            Ok(Some(row)) => {
                let device_id = origin_or(&row.origin_device_id, &self.state.device_id).to_string();
                let names = self
                    .state
                    .store
                    .device_names(std::slice::from_ref(&device_id))
                    .unwrap_or_default();
                self.to_wire_with(row, &names)
            }
            Ok(None) => Err(BackendError::NotFound(MSG_NO_ITEM)),
            Err(_) => Err(BackendError::internal("history could not be read")),
        }
    }

    pub(super) fn image_preview(&self, id: &str) -> Result<ImagePreview> {
        let row = match self.state.store.get(id) {
            Ok(Some(row)) => row,
            Ok(None) => return Err(BackendError::NotFound(MSG_NO_ITEM)),
            Err(_) => return Err(BackendError::internal("history could not be read")),
        };
        if row.is_sensitive || !row.content_type.starts_with("image/") {
            return Err(BackendError::Invalid("That image preview is unavailable."));
        }
        let bytes = open_binary(
            &row.content_ciphertext,
            &self.state.keyring.item_key(),
            &row.id,
        )
        .map_err(|_| BackendError::internal("that item could not be decrypted"))?;
        let thumbnail = thumbnail_png(&bytes, self.settings().max_decoded_image_mb)
            .map_err(|_| BackendError::Invalid("That image preview is unavailable."))?;
        Ok(ImagePreview {
            png_base64: STANDARD.encode(thumbnail.png),
            width: thumbnail.width,
            height: thumbnail.height,
        })
    }
}

fn is_oversized_text(payload: &ClipboardPayload) -> bool {
    matches!(payload, ClipboardPayload::Text(_))
        && payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES
}

/// The status this build can honestly report.
///
/// Never fails: an unreadable count is reported as zero rather than an error,
/// because a caller may be probing precisely because storage is unhappy.
pub(super) fn status_of(inner: &Inner) -> Result<copypaste_ipc::StatusData> {
    let settings = inner.state.settings.snapshot();
    Ok(copypaste_ipc::StatusData {
        device_name: inner.state.device_name(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: copypaste_ipc::PROTOCOL_VERSION,
        listen_addr: None,
        device_details: None,
        item_count: inner.state.store.count().unwrap_or(0),
        // There is no capture loop in this build: Android has no background
        // daemon and no clipboard polling. Reporting `true` would tell the
        // status line that history is growing when it is not.
        capture_running: false,
        clipboard_backend: super::messages::BACKEND_NAME.to_string(),
        private_mode: settings.config.private_mode,
        private_mode_epoch: settings.private_mode_epoch,
        // Android has no daemon poller, but it does run the same startup FTS
        // purge as the daemon. Surface that one counter rather than claiming
        // the purge never happened.
        counters: copypaste_ipc::DiagnosticCounters {
            index_purged: inner.state.index_purged,
            ..Default::default()
        },
        // Android has no daemon, but it does have a persisted settings record
        // and it does fail closed on one it cannot read (DMY155-B2). Reporting
        // `None` here would leave the user running on privacy values they never
        // chose with nothing on screen saying so.
        settings_health: settings.health.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;

    #[tokio::test]
    async fn a_row_moved_under_another_identity_fails_closed_and_redacted() {
        let (backend, _clipboard, _dir) = backend();
        let item = backend.add("plaintext that must not escape").await.unwrap();
        let mut row = backend
            .inner
            .state
            .store
            .get(&item.id)
            .unwrap()
            .expect("the row was stored");
        row.id = "another-row".into();

        let error = backend.inner.to_wire(row).unwrap_err();
        assert!(matches!(error, BackendError::Internal(_)), "{error:?}");
        let shown = serde_json::to_string(&error.ui_error()).unwrap();
        assert!(!shown.contains("plaintext that must not escape"), "{shown}");
        assert!(!shown.contains(&item.id), "{shown}");
    }
}
