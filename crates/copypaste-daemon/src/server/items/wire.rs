//! Conversion from encrypted stored rows to IPC items.

use copypaste_core::{ClipboardPayload, StoredItem};
use copypaste_ipc::{Item, ItemPage};
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
    copypaste_core::history_actions::oversized_text(payload)
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
    let origin = origin_identity_for(state, &row);
    to_wire_with(
        row,
        &origin,
        state.meta.device_id(),
        &state.keyring.item_key(),
        false,
    )
}

pub(super) fn origin_identity_for(state: &AppState, row: &StoredItem) -> crate::meta::Origin {
    let origin = state.meta.origin_of(row).unwrap_or_else(|e| {
        warn!(error = ?e, "could not resolve an item's origin device");
        origin_identity(state, row)
    });
    origin
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
use copypaste_core::history_item::open_item as to_wire_with;

/// Decrypt a page of rows, dropping any row that will not open — and saying how
/// many.
///
/// One unreadable row must not blank an entire page of history: the other items
/// are still the user's data. `CopyPaste-00zz`: a shortened page must carry the
/// unreadable count instead of explaining it only in logs. The
/// count goes back on the wire so a client can say "3 items could not be read".
pub(super) fn preview_rows(
    state: &AppState,
    rows: Vec<copypaste_core::HistoryPreviewRow>,
) -> ItemPage {
    let (rows, previews): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .map(|entry| (entry.row, entry.preview))
        .unzip();
    let origins = state.meta.origins_for(&rows).unwrap_or_else(|error| {
        warn!(?error, "could not resolve origin devices for a page");
        std::collections::HashMap::new()
    });
    let mut page = ItemPage {
        items: Vec::with_capacity(rows.len()),
        skipped_undecryptable: 0,
        next_cursor: None,
    };
    for (row, preview) in rows.into_iter().zip(previews) {
        let Some(preview) = preview else {
            page.skipped_undecryptable += 1;
            continue;
        };
        let fallback;
        let origin = match origins.get(&row.id) {
            Some(origin) => origin,
            None => {
                fallback = origin_identity(state, &row);
                &fallback
            }
        };
        page.items
            .push(preview.into_item(&row, origin, state.meta.device_id()));
    }
    page
}

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
