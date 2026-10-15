//! Conversion from encrypted stored rows to IPC items.

use copypaste_core::{ClipboardPayload, StoredItem};
use copypaste_ipc::{FileDetails, ImageDetails, Item, ItemPage};
use tracing::warn;

use crate::AppState;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_previews_require_an_explicit_secret_hint_to_hide_content() {
        let (state, _dir) = crate::testutil::test_state("email-preview");
        let text = "person.99@example.com";
        let id = crate::testutil::add(&state, text);
        let stored = state.store.get(&id).unwrap().unwrap();
        for (metadata, secret) in [
            ("unrecognized old metadata", false),
            (
                r#"{"privacy":{"secret":true},"source_app_icon":"broken"}"#,
                true,
            ),
        ] {
            let mut row = stored.clone();
            row.payload_metadata = Some(metadata.into());
            for preview in [true, false] {
                let (item, _) = to_wire_with(
                    row.clone(),
                    &state.meta.here(),
                    state.meta.device_id(),
                    &state.keyring.item_key(),
                    preview,
                )
                .unwrap();
                assert_eq!(item.privacy.secret, secret);
                if secret && preview {
                    assert!(item.content.is_empty());
                    assert!(item.semantic_kind.is_none());
                } else {
                    assert_eq!(item.content, text);
                    assert_eq!(item.semantic_kind, Some(copypaste_ipc::SemanticKind::Email));
                }
            }
        }
    }

    #[test]
    fn origin_lookup_errors_preserve_remote_identity_and_local_file_availability() {
        for table in ["sync_device_name", "sync_device_state"] {
            let (state, dir) = crate::testutil::test_state("origin-fallback");
            let source = dir.path().join("remote-original.txt");
            std::fs::write(&source, b"file bytes").unwrap();
            let metadata = copypaste_core::FileMetadata::with_source_reference(
                "remote-original.txt",
                "text/plain",
                source.to_str().unwrap(),
            )
            .unwrap();
            let mut remote = copypaste_core::ingest::ingest_binary_into_with_capture_context(
                &state.store,
                &state.keyring,
                b"file bytes",
                copypaste_ipc::content_type::FILE,
                copypaste_core::now_ms(),
                None,
                Some(&metadata),
                &state.settings.get(),
            )
            .unwrap()
            .into_item();
            remote.origin_device_id = "remote-device".into();
            let local_id = crate::testutil::add(&state, "local text");
            let local = state.store.get(&local_id).unwrap().unwrap();
            let conn =
                copypaste_core::storage::open_validated(state.db_path(), &state.keyring.db_key())
                    .unwrap();
            conn.execute(&format!("DROP TABLE {table}"), []).unwrap();
            assert!(state.meta.origin_of(&remote).is_err());

            let item = to_wire(&state, remote.clone()).unwrap();
            assert_eq!(item.origin_device_id, "remote-device");
            assert_eq!(item.origin_device_name, None);
            assert_eq!(
                item.origin_device_class,
                copypaste_ipc::DeviceClass::Unknown
            );
            assert!(!item.file_details.unwrap().source_available);

            let page = decrypt_rows(&state, vec![remote, local]);
            assert_eq!(page.skipped_undecryptable, 0);
            assert_eq!(page.items[0].origin_device_id, "remote-device");
            assert!(
                !page.items[0]
                    .file_details
                    .as_ref()
                    .unwrap()
                    .source_available
            );
            assert_eq!(page.items[1].origin_device_id, state.meta.device_id());
            assert_eq!(
                page.items[1].origin_device_name.as_deref(),
                Some(state.meta.device_name().as_str()),
            );
        }
    }
}

pub(super) fn is_oversized_text(payload: &ClipboardPayload) -> bool {
    matches!(payload, ClipboardPayload::Text(_))
        && payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES
}

pub(super) fn bound_item_preview(item: &mut Item) {
    item.truncated = copypaste_ipc::limits::bound_preview(&mut item.content);
}

/// Decrypt a stored row into its wire form, resolving its origin as it goes.
///
/// [`decrypt_rows`] batches origin lookup for a whole page.
pub(super) fn to_wire(
    state: &AppState,
    row: StoredItem,
) -> Result<Item, copypaste_core::CryptoError> {
    to_wire_and_payload(state, row).map(|(item, _)| item)
}

pub(super) fn to_wire_and_payload(
    state: &AppState,
    row: StoredItem,
) -> Result<(Item, ClipboardPayload), copypaste_core::CryptoError> {
    let origin = state.meta.origin_of(&row).unwrap_or_else(|e| {
        warn!(error = ?e, "could not resolve an item's origin device");
        origin_identity(state, &row)
    });
    to_wire_with(
        row,
        &origin,
        state.meta.device_id(),
        &state.keyring.item_key(),
        false,
    )
}

fn origin_identity(state: &AppState, row: &StoredItem) -> crate::meta::Origin {
    let device_id = copypaste_core::origin_or(&row.origin_device_id, state.meta.device_id());
    if device_id == state.meta.device_id() {
        state.meta.here()
    } else {
        crate::meta::Origin {
            device_id: device_id.to_owned(),
            device_name: None,
            device_class: copypaste_ipc::DeviceClass::Unknown,
        }
    }
}

/// Convert with the origin and item key already resolved.
///
/// The plaintext accompanies the item so `copy` writes the exact bytes
/// authenticated for this row; no entry point accepts externally opened bytes.
fn to_wire_with(
    row: StoredItem,
    origin: &crate::meta::Origin,
    local_device_id: &str,
    key: &copypaste_core::ItemKey,
    preview: bool,
) -> Result<(Item, ClipboardPayload), copypaste_core::CryptoError> {
    // The item id is the AAD: a row decrypted under another row's identity must
    // fail authentication, not fall back to a plaintext read (AGENTS.md rule 4,
    // "fail closed on crypto").
    let privacy = row.clipboard_privacy();
    let payload = ClipboardPayload::open(&row, key)?;
    // Measured on the plaintext bytes, because that is what the cloud path
    // measures: `LocalItem::content` is the opened payload, and the seal that
    // follows is a fixed overhead the cap does not count.
    let too_large_to_sync =
        copypaste_sync::too_large_to_sync(&row.content_type, payload.byte_len());
    let (content, truncated) = if preview && privacy.secret {
        (String::new(), false)
    } else if preview {
        payload.display_preview()
    } else {
        (payload.display_text(), false)
    };
    let imported_image_metadata = row
        .payload_metadata
        .as_deref()
        .and_then(|value| copypaste_core::PayloadMetadata::from_json(value, &row.content_type))
        .and_then(|value| value.file);
    let file_details = match &payload {
        ClipboardPayload::File { bytes, .. } | ClipboardPayload::Image { bytes, .. }
            if imported_image_metadata.is_some()
                || matches!(&payload, ClipboardPayload::File { .. }) =>
        {
            let metadata = match &payload {
                ClipboardPayload::File { metadata, .. } => metadata,
                _ => &imported_image_metadata,
            };
            let source_reference = metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.clone());
            Some(FileDetails {
                filename: metadata.as_ref().map(|metadata| metadata.filename.clone()),
                mime_type: metadata.as_ref().map(|metadata| metadata.mime_type.clone()),
                source_available: origin.device_id == local_device_id
                    && source_reference
                        .as_deref()
                        .is_some_and(|reference| std::path::Path::new(reference).is_file()),
                source_reference,
                size_bytes: bytes.len() as u64,
                file_count: 1,
            })
        }
        ClipboardPayload::File { .. }
        | ClipboardPayload::Text(_)
        | ClipboardPayload::Image { .. }
        | ClipboardPayload::Unsupported { .. } => None,
    };
    let image_details = if preview {
        None
    } else {
        match &payload {
            ClipboardPayload::Image { bytes, .. } => copypaste_core::image_metadata(bytes)
                .ok()
                .map(|metadata| ImageDetails {
                    width: metadata.width,
                    height: metadata.height,
                    size_bytes: metadata.size_bytes,
                }),
            ClipboardPayload::Text(_)
            | ClipboardPayload::File { .. }
            | ClipboardPayload::Unsupported { .. } => None,
        }
    };
    let semantic = payload
        .plain_text()
        .and_then(|text| copypaste_core::classify_semantic(&row.content_type, text));
    let content_class = copypaste_ipc::content_type::classify(&row.content_type);
    let item = Item {
        privacy,
        id: row.id,
        content,
        content_type: row.content_type,
        content_class,
        semantic_kind: semantic
            .filter(|_| !privacy.secret || !preview)
            .map(|classification| classification.kind),
        color_rgba: semantic
            .filter(|_| !privacy.secret || !preview)
            .and_then(|classification| classification.color_rgba),
        created_at: row.created_at,
        pinned: row.pinned,
        file_details: if preview && privacy.secret {
            None
        } else {
            file_details
        },
        image_details,
        origin_device_id: origin.device_id.clone(),
        origin_device_name: origin.device_name.clone(),
        origin_device_class: origin.device_class,
        source_app_bundle_id: row.app_bundle_id,
        source_app_name: row.app_name,
        source_app_icon_id: row.source_icon_id,
        too_large_to_sync,
        truncated,
    };
    Ok((item, payload))
}

/// Decrypt a page of rows, dropping any row that will not open — and saying how
/// many.
///
/// One unreadable row must not blank an entire page of history: the other items
/// are still the user's data. `CopyPaste-00zz`: a shortened page must carry the
/// unreadable count instead of explaining it only in logs. The
/// count goes back on the wire so a client can say "3 items could not be read".
pub(super) fn decrypt_rows(state: &AppState, rows: Vec<StoredItem>) -> ItemPage {
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
    // One HKDF extract-and-expand for the page, not one per row: the derivation
    // is constant for the process. Scoped to the page rather than held on
    // `AppState`, so the key's window in memory is no wider than the plaintexts
    // it opens.
    let key = state.keyring.item_key();
    for row in rows {
        let row_id = row.id.clone();
        let fallback;
        let origin = match origins.get(&row_id) {
            Some(origin) => origin,
            None => {
                fallback = origin_identity(state, &row);
                &fallback
            }
        };
        match to_wire_with(row, origin, state.meta.device_id(), &key, true) {
            Ok((item, _)) => page.items.push(item),
            Err(e) => {
                warn!(id = %row_id, error = ?e, "skipping an item that failed to decrypt");
                page.skipped_undecryptable += 1;
            }
        }
    }
    page
}
