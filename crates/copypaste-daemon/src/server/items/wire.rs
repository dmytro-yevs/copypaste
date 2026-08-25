//! Conversion from encrypted stored rows to IPC items.

use copypaste_core::{ClipboardPayload, StoredItem};
use copypaste_ipc::{Item, ItemPage};
use tracing::warn;

use crate::AppState;

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
        // Attribution is advisory: a row whose origin cannot be read is still
        // the user's item, and the fallback is the same one the origin table's
        // absence already means — this device captured it.
        warn!(error = ?e, "could not resolve an item's origin device");
        state.meta.here()
    });
    to_wire_with(row, &origin, &state.keyring.item_key(), &state.detector)
}

/// Convert with the origin and item key already resolved.
///
/// The plaintext accompanies the item so `copy` writes the exact bytes
/// authenticated for this row; no entry point accepts externally opened bytes.
fn to_wire_with(
    row: StoredItem,
    origin: &crate::meta::Origin,
    key: &copypaste_core::ItemKey,
    detector: &copypaste_core::Detector,
) -> Result<(Item, ClipboardPayload), copypaste_core::CryptoError> {
    // The item id is the AAD: a row decrypted under another row's identity must
    // fail authentication, not fall back to a plaintext read (AGENTS.md rule 4,
    // "fail closed on crypto").
    let payload = ClipboardPayload::open(&row, key)?;
    // Measured on the plaintext bytes, because that is what the cloud path
    // measures: `LocalItem::content` is the opened payload, and the seal that
    // follows is a fixed overhead the cap does not count.
    let too_large_to_sync =
        copypaste_cloud::sync::too_large_to_sync(&row.content_type, payload.byte_len());
    let content = payload.display_text();
    let sensitive_finding = (!row.is_sensitive
        && copypaste_ipc::content_type::is_text(&row.content_type))
    .then(|| detector.inert_finding_metadata(&content))
    .flatten();
    let item = Item {
        id: row.id,
        content,
        content_type: row.content_type,
        created_at: row.created_at,
        pinned: row.pinned,
        is_sensitive: row.is_sensitive,
        sensitive_finding,
        origin_device_id: origin.device_id.clone(),
        origin_device_name: origin.device_name.clone(),
        source_app_bundle_id: row.app_bundle_id,
        source_app_name: row.app_name,
        too_large_to_sync,
        truncated: false,
    };
    Ok((item, payload))
}

/// Decrypt a page of rows, dropping any row that will not open — and saying how
/// many.
///
/// One unreadable row must not blank an entire page of history: the other items
/// are still the user's data. But a page that is silently one item shorter, with
/// the reason only in the daemon's log, is what v1 shipped and what
/// `CopyPaste-00zz` fixed — the user sees fewer items and is told nothing. The
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
    let here = state.meta.here();
    // One HKDF extract-and-expand for the page, not one per row: the derivation
    // is constant for the process. Scoped to the page rather than held on
    // `AppState`, so the key's window in memory is no wider than the plaintexts
    // it opens.
    let key = state.keyring.item_key();
    for row in rows {
        let row_id = row.id.clone();
        let origin = origins.get(&row_id).unwrap_or(&here);
        match to_wire_with(row, origin, &key, &state.detector) {
            Ok((item, _)) => page.items.push(item),
            Err(e) => {
                warn!(id = %row_id, error = ?e, "skipping an item that failed to decrypt");
                page.skipped_undecryptable += 1;
            }
        }
    }
    page
}
