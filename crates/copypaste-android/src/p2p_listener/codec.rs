//! Wire codec — item ↔ crypto translation between the local item store and
//! the on-wire `WireItem` frames.

use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud, SyncKey};
use copypaste_sync::protocol::WireItem;

use crate::{CopypasteError, LocalItem, SyncedItem, P2P_WIRE_KEY_VERSION};

/// Re-key the local history into outbound `WireItem`s under `shared`, mirroring
/// `sync_with_peer`'s catch-up build EXACTLY (same wire contract: the cloud
/// blob lives in `content`, `content_nonce` is `None`, image/file types carried
/// through). `device_id` stamps the origin so the peer can dedup by origin.
pub(super) fn build_catchup_wire_items(
    local_items: &[LocalItem],
    shared: &SyncKey,
    device_id: &str,
) -> Result<Vec<WireItem>, CopypasteError> {
    let mut outbound: Vec<WireItem> = Vec::with_capacity(local_items.len());
    for it in local_items {
        // Tombstones (deleted=true): emit a WireItem with no content blob so
        // the peer applies the delete via LWW without decrypting anything.
        if it.deleted {
            let item_id = if it.item_id.is_empty() {
                it.id.clone()
            } else {
                it.item_id.clone()
            };
            let id = if it.id.is_empty() {
                item_id.clone()
            } else {
                it.id.clone()
            };
            outbound.push(WireItem {
                id,
                item_id,
                content_type: it.content_type.clone(),
                content: None,
                content_nonce: None,
                blob_ref: None,
                is_sensitive: false,
                lamport_ts: it.wall_time_ms,
                wall_time: it.wall_time_ms,
                expires_at: None,
                app_bundle_id: None,
                origin_device_id: device_id.to_string(),
                key_version: P2P_WIRE_KEY_VERSION,
                file_name: None,
                mime: None,
                deleted: true,
                pinned: it.pinned,
                pin_order: it.pin_order,
            });
            continue;
        }
        let wire_content_type = if it.content_type == "text" || it.content_type.starts_with("text/")
        {
            "text".to_string()
        } else if it.content_type == "image" || it.content_type.starts_with("image/") {
            it.content_type.clone()
        } else if it.content_type == "file" {
            "file".to_string()
        } else {
            continue;
        };
        // STABLE identity: reuse the caller's persisted item_id; fall back to
        // the row id only for transitional rows. Never mint a fresh UUID here.
        let item_id = if it.item_id.is_empty() {
            it.id.clone()
        } else {
            it.item_id.clone()
        };
        let id = if it.id.is_empty() {
            item_id.clone()
        } else {
            it.id.clone()
        };
        let blob = encrypt_for_cloud(shared, &item_id, &it.plaintext)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        outbound.push(WireItem {
            id,
            item_id,
            content_type: wire_content_type,
            content: Some(blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: it.wall_time_ms,
            wall_time: it.wall_time_ms,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: device_id.to_string(),
            key_version: P2P_WIRE_KEY_VERSION,
            file_name: it.file_name.clone(),
            mime: it.mime.clone(),
            // Propagate caller-supplied pin state to the wire.
            deleted: false,
            pinned: it.pinned,
            pin_order: it.pin_order,
        });
    }
    Ok(outbound)
}

/// Decrypt one inbound `WireItem` into a [`SyncedItem`], or `None` if it is a
/// legacy/non-rekeyed frame, an unknown content type, or fails to decrypt with
/// the shared key. Mirrors the inbound unwrap in `sync_with_peer` — never logs
/// key bytes.
///
/// ABI 14: tombstone frames (`deleted == true`) are surfaced immediately with
/// empty plaintext so Kotlin can apply/refresh the local tombstone via LWW.
pub(super) fn decrypt_wire_item(wire: &WireItem, shared: &SyncKey) -> Option<SyncedItem> {
    // ABI 14: tombstone frame — surface it without decryption.
    if wire.deleted {
        return Some(SyncedItem {
            id: wire.id.clone(),
            item_id: wire.item_id.clone(),
            content_type: wire.content_type.clone(),
            plaintext: Vec::new(),
            wall_time_ms: wire.wall_time,
            file_name: None,
            mime: None,
            deleted: true,
            pinned: wire.pinned,
            pin_order: wire.pin_order,
        });
    }
    // A text frame that still carries a content_nonce is a legacy / non-rekeyed
    // frame we cannot decrypt with the shared sync key — skip it.
    if wire.content_type == "text" && wire.content_nonce.is_some() {
        return None;
    }
    let is_text = wire.content_type == "text" || wire.content_type.starts_with("text/");
    let is_image = wire.content_type == "image" || wire.content_type.starts_with("image/");
    let is_file = wire.content_type == "file";
    if !(is_text || is_image || is_file) {
        return None;
    }
    let blob = wire.content.as_ref()?;
    match decrypt_from_cloud(shared, &wire.item_id, blob) {
        Ok(plaintext) => Some(SyncedItem {
            id: wire.id.clone(),
            item_id: wire.item_id.clone(),
            content_type: wire.content_type.clone(),
            plaintext,
            wall_time_ms: wire.wall_time,
            file_name: wire.file_name.clone(),
            mime: wire.mime.clone(),
            // ABI 14: propagate pin state from the wire.
            deleted: false,
            pinned: wire.pinned,
            pin_order: wire.pin_order,
        }),
        Err(_) => None,
    }
}
