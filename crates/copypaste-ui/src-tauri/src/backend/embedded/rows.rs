//! Stored rows into wire items.
//!
//! Separated from the command surface in `super` so that "what a row becomes"
//! is one small thing to read: the AAD binding and the count of what would not
//! open are the whole file.

use std::collections::HashMap;

use copypaste_core::{decrypt, origin_or, StoredItem};
use copypaste_ipc::{Item, PeerInfo};

use super::Inner;
use crate::backend::{BackendError, Page, Result};

pub(super) use copypaste_ipc::{clamp_page, DEFAULT_LIST_PAGE, DEFAULT_SEARCH_PAGE};

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
        legacy_history_present: legacy_history_present(&inner.data_dir),
        // All zero, and honestly so: this build has no capture loop to refuse
        // or miss anything, and no startup purge. A count it cannot collect is
        // reported as none rather than left out.
        counters: copypaste_ipc::DiagnosticCounters::default(),
    })
}

/// Whether a CopyPaste 0.4 history is inside this sandbox (B-33).
///
/// Read-only: `v1_database_in` opens nothing with a key and writes nothing, so
/// a downgrade finds the file exactly as it was (CLAUDE.md rule 3).
///
/// Three directories rather than one. `copypaste_ipc::v1_data_dir` cannot
/// address an Android sandbox and this build was handed the one it needs — but
/// `app_data_dir()` is `Context.dataDir`, the sandbox *root*, and an Android app
/// puts a database under `databases/` or `files/`. No manifest records which
/// v0.4 used, so none is guessed at. A wrong hit is not the hazard here:
/// `v1_database_in` answers on v0.4's schema or on its exact filename plus a
/// page-aligned size, and v2 writes neither.
fn legacy_history_present(data_dir: &std::path::Path) -> bool {
    const SUBDIRS: [&str; 2] = ["databases", "files"];
    std::iter::once(data_dir.to_path_buf())
        .chain(SUBDIRS.iter().map(|name| data_dir.join(name)))
        .any(|dir| copypaste_core::v1_database_in(&dir))
}

#[cfg(test)]
mod tests {
    use crate::backend::embedded::tests::backend;
    use crate::backend::Backend as _;

    /// v2's own directory holds `copypaste-v2.db`, a keyring and a peer file,
    /// and none of them is a v0.4 history. The negative has to hold first: a
    /// probe that answered yes here would put the banner in front of every user
    /// who never ran 0.4.
    #[tokio::test]
    async fn a_directory_this_build_made_itself_is_not_a_v0_4_history() {
        let (backend, _clip, _dir) = backend();
        backend.add("something").await.unwrap();
        assert!(!backend.status().await.unwrap().legacy_history_present);
    }

    /// B-33. `clipboard.db` under this build's own data directory is the case
    /// the Android hard-coded `false` hid: the file is there, this build cannot
    /// read it, and until now nothing said so.
    ///
    /// Staged as SQLCipher-shaped bytes rather than a v0.4 schema because an
    /// encrypted v0.4 file is what an Android upgrade actually leaves behind,
    /// and v2 cannot derive v0.4's key to write a real one.
    ///
    /// Run for each candidate directory, because `app_data_dir()` is the
    /// sandbox root and the two conventional places are where a v0.4 file would
    /// really be — a probe that only asked the root would answer "no history"
    /// for the case B-33 is about.
    #[tokio::test]
    async fn a_v0_4_history_anywhere_in_the_sandbox_is_reported_and_left_untouched() {
        for subdir in ["", "databases", "files"] {
            let (backend, _clip, dir) = backend();
            let at = dir.path().join(subdir);
            std::fs::create_dir_all(&at).unwrap();
            let legacy = at.join(copypaste_core::V1_DATABASE_FILENAME);
            std::fs::write(&legacy, vec![0x5au8; 3 * 4096]).unwrap();
            let before = std::fs::read(&legacy).unwrap();

            let status = backend.status().await.unwrap();
            assert!(status.legacy_history_present, "missed one under {subdir:?}");

            // Rule 3's obligation, checked the way the desktop probe checks it:
            // a user who downgrades finds the file exactly as they left it.
            assert_eq!(std::fs::read(&legacy).unwrap(), before, "the file changed");
            for suffix in ["-wal", "-shm", "-journal"] {
                let sidecar = at.join(format!("clipboard.db{suffix}"));
                assert!(!sidecar.exists(), "the probe wrote {suffix}");
            }
        }
    }
}
