//! Stored rows into wire items, and the page bounds both platforms share.
//!
//! Separated from the command surface in `super` so that "what a row becomes"
//! is one small thing to read: the AAD binding, the count of what would not
//! open, and the two clamps are the whole file.

use std::collections::HashMap;

use copypaste_core::{decrypt, origin_or, StoredItem};
use copypaste_ipc::{Item, PeerInfo};

use super::Inner;
use crate::backend::{BackendError, Page, Result};

/// Server-side clamp on a caller-supplied page size (manifest 04 §3.3).
/// Identical to the daemon's, because it is the same contract seen from the
/// other side — a frontend asking for ten million rows must get the same
/// answer on both platforms.
pub(super) const MAX_PAGE: u32 = 1_000;
pub(super) const DEFAULT_LIST_PAGE: u32 = 50;
pub(super) const DEFAULT_SEARCH_PAGE: u32 = 20;

const MSG_NO_ITEM: &str = "That item is no longer there.";

impl Inner {
    /// Decrypt one stored row into its wire form.
    ///
    /// Two library calls and a struct literal. The item id is the AAD, so a row
    /// decrypted under another row's identity fails authentication rather than
    /// falling back to a plaintext read (CLAUDE.md rule 4, "fail closed").
    pub(super) fn to_wire(&self, row: StoredItem) -> Result<Item> {
        let device_id = origin_or(&row.origin_device_id, &self.device_id).to_string();
        let names = self
            .store
            .device_names(std::slice::from_ref(&device_id))
            .unwrap_or_default();
        self.to_wire_with(row, &names)
    }

    /// [`Inner::to_wire`] with the page's device names already resolved.
    fn to_wire_with(&self, row: StoredItem, names: &HashMap<String, String>) -> Result<Item> {
        let key = self.keyring.item_key();
        let plaintext = decrypt(&row.content_ciphertext, &row.nonce, &key, &row.id)
            .map_err(|_| BackendError::internal("that item could not be decrypted"))?;
        // A row captured here stores no origin; substituting this device's id
        // is what makes the field mean the same thing on both platforms
        // (`copypaste_core::origin_or`). The name is `None` rather than a guess
        // until a session with that device has told us one.
        let device_id = origin_or(&row.origin_device_id, &self.device_id).to_string();
        let origin_device_name = names.get(&device_id).cloned();
        Ok(Item {
            id: row.id,
            content: String::from_utf8_lossy(&plaintext).into_owned(),
            content_type: row.content_type,
            created_at: row.created_at,
            pinned: row.pinned,
            is_sensitive: row.is_sensitive,
            origin_device_id: device_id,
            origin_device_name,
            // Nothing here talks to a cloud account.
            too_large_to_sync: false,
        })
    }

    /// Decrypt a page, dropping any row that will not open — and **counting**
    /// what was dropped.
    ///
    /// One unreadable row must not blank a whole page: the other items are
    /// still the user's data (CLAUDE.md rule 4). But dropping it silently makes
    /// a short page indistinguishable from a small history, which is parity
    /// finding 17 / `CopyPaste-00zz`. The count is what lets the UI say "3
    /// items could not be read" instead of showing three fewer rows.
    pub(super) fn to_wire_page(&self, rows: Vec<StoredItem>) -> Page {
        // One name query for the page rather than one per row: a page is up to
        // `MAX_PAGE` items and this runs on every list and every search.
        let device_ids: Vec<String> = rows
            .iter()
            .map(|row| origin_or(&row.origin_device_id, &self.device_id).to_string())
            .collect();
        let names = self.store.device_names(&device_ids).unwrap_or_default();

        let mut page = Page::default();
        for row in rows {
            let id = row.id.clone();
            match self.to_wire_with(row, &names) {
                Ok(item) => page.items.push(item),
                Err(_) => {
                    tracing::warn!(%id, "skipping an item that failed to decrypt");
                    page.skipped_undecryptable = page.skipped_undecryptable.saturating_add(1);
                }
            }
        }
        page
    }

    pub(super) fn fetch(&self, id: &str) -> Result<Item> {
        match self.store.get(id) {
            Ok(Some(row)) => self.to_wire(row),
            Ok(None) => Err(BackendError::NotFound(MSG_NO_ITEM)),
            Err(_) => Err(BackendError::internal("history could not be read")),
        }
    }
}

pub(super) fn clamp_page(limit: u32, default: u32) -> u32 {
    if limit == 0 {
        default
    } else {
        limit.min(MAX_PAGE)
    }
}

pub(super) fn peer_info(peer: &copypaste_p2p::Peer, online: bool) -> PeerInfo {
    PeerInfo {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        // `Peer` holds a parsed `SocketAddr`; the wire type carries the
        // rendered form, because it is shown to a user and never dialled by the
        // frontend.
        last_addr: peer.last_addr.map(|addr| addr.to_string()),
        last_seen_ms: peer.last_seen_ms,
        online,
    }
}

/// The status this build can honestly report.
///
/// Never fails: an unreadable count is reported as zero rather than an error,
/// because a caller may be probing precisely because storage is unhappy.
pub(super) fn status_of(inner: &Inner) -> Result<copypaste_ipc::StatusData> {
    Ok(copypaste_ipc::StatusData {
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: copypaste_ipc::PROTOCOL_VERSION,
        item_count: inner.store.count().unwrap_or(0),
        // There is no capture loop in this build: Android has no background
        // daemon and no clipboard polling. Reporting `true` would tell the
        // status line that history is growing when it is not.
        capture_running: false,
        clipboard_backend: super::BACKEND_NAME.to_string(),
        // v0.4's Android history lived inside the app sandbox, which
        // `copypaste_ipc::v1_data_dir` cannot address — so this build has no
        // way to look, and says so by saying no.
        legacy_history_present: false,
        // All zero, and honestly so: this build has no capture loop to refuse
        // or miss anything, and no startup purge. A count it cannot collect is
        // reported as none rather than left out.
        counters: copypaste_ipc::DiagnosticCounters::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_sizes_are_clamped_exactly_as_the_daemon_clamps_them() {
        assert_eq!(clamp_page(0, DEFAULT_LIST_PAGE), DEFAULT_LIST_PAGE);
        assert_eq!(clamp_page(10, DEFAULT_LIST_PAGE), 10);
        assert_eq!(clamp_page(u32::MAX, DEFAULT_LIST_PAGE), MAX_PAGE);
    }
}
