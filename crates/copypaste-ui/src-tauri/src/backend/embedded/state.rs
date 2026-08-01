//! Persistent state and startup repair for the in-process backend.
//!
//! The v2 store, its keys, the device identity and the detector are opened as
//! one unit. The search-index purge is best-effort: losing a stale FTS row is
//! better than blocking access to encrypted history when SQLite cannot read
//! the index.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use copypaste_core::{purge_indexed_secrets, Detector, Keyring, Store};

use crate::backend::{BackendError, Result};

/// What this device tells peers it is called, on first run only.
///
/// Cosmetic and peer-visible. Android has no hostname worth reading — it is
/// `localhost` — and there is no rename screen yet, so this is the honest
/// placeholder rather than a guess dressed up as a device name.
const DEVICE_NAME_HINT: &str = "CopyPaste phone";

pub(super) struct BackendState {
    pub(super) store: Store,
    /// Kept for the v0.4 probe in [`super::rows::status_of`], which states why
    /// (B-33).
    pub(super) data_dir: PathBuf,
    // Both are behind an `Arc` because `copypaste_core::StoreSource` holds them
    // for as long as the peer listener runs.
    pub(super) keyring: Arc<Keyring>,
    pub(super) detector: Arc<Detector>,
    /// This device's sync identity, resolved once from the history database and
    /// then fixed: merge key 4 and every hello depend on it not moving.
    pub(super) device_id: String,
    pub(super) device_name: String,
    /// Where the paired-device list lives. The `PeerStore` itself belongs to
    /// the node, which owns it by value.
    pub(super) peers_path: PathBuf,
    /// The defaults, and only the defaults: there is no daemon here and no
    /// config file, so a settings screen on Android will not stick (ADR-0005).
    pub(super) settings: copypaste_ipc::ConfigData,
    /// Search-index rows removed by this process's startup purge. Kept so the
    /// status/diagnostics surface tells the same truth as the daemon does.
    pub(super) index_purged: u64,
}

impl BackendState {
    /// Open persistent state under the Android context's data directory.
    pub(super) fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| BackendError::internal(&format!("could not prepare storage: {e}")))?;

        // The same directory the database goes in, so the secret cannot end up
        // somewhere the history is not (security review F-11).
        let keyring = Keyring::load_or_create(data_dir)
            .map_err(|e| BackendError::internal(&format!("could not open the keystore: {e}")))?;

        // The v2 filename, from the shared crate. Deliberately distinct from
        // v1's so an old database is never touched (CLAUDE.md rule 3).
        let db_path = data_dir.join(
            copypaste_ipc::database_path()
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("copypaste-v2.db")),
        );
        let store = Store::open(&db_path, &keyring.db_key())
            .map_err(|e| BackendError::internal(&format!("could not open history: {e}")))?;

        // Minted on first run, in the history database, so it moves with the
        // history and is the same identity a restored backup keeps out of.
        let identity = store
            .device_identity(DEVICE_NAME_HINT)
            .map_err(|e| BackendError::internal(&format!("could not resolve this device: {e}")))?;

        let detector = Detector::new()
            .map_err(|e| BackendError::internal(&format!("could not build the detector: {e}")))?;
        // The third enforcement layer: a detector rule can be added after a
        // row entered FTS. This only removes the plaintext index entry, never
        // history, so failure is logged rather than preventing the app from
        // opening the user's clips (the daemon follows the same rule).
        let index_purged = purge_search_index(&store, &detector);

        Ok(Self {
            store,
            data_dir: data_dir.to_path_buf(),
            keyring: Arc::new(keyring),
            detector: Arc::new(detector),
            device_id: identity.device_id,
            device_name: identity.device_name,
            // The name from the shared crate, as the daemon uses. Spelling it
            // here is how this backend came to open `peers.json`, which is v1's
            // file (CLAUDE.md rule 3).
            peers_path: data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
            settings: copypaste_ipc::ConfigData::default(),
            index_purged,
        })
    }
}

fn purge_search_index(store: &Store, detector: &Detector) -> u64 {
    match purge_indexed_secrets(store, detector) {
        Ok(report) => {
            if report.purged > 0 {
                tracing::info!(
                    purged = report.purged,
                    scanned = report.scanned,
                    "removed search-index rows the current ruleset calls sensitive"
                );
            }
            report.purged
        }
        Err(error) => {
            tracing::warn!(%error, "the search-index purge did not finish");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;

    #[test]
    fn the_embedded_open_purge_removes_previously_indexed_sensitive_text() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("copypaste-v2.db"), &[7; 32]).unwrap();
        let text = "mail alice.smith@example.com about it";
        store
            .insert(copypaste_core::NewItem {
                id: "old-sensitive-index-row".to_string(),
                content_ciphertext: Vec::new(),
                nonce: Vec::new(),
                content_type: "text/plain".to_string(),
                content_hash: "old-sensitive-index-row".to_string(),
                is_sensitive: false,
                search_text: Some(text.to_string()),
                app_bundle_id: None,
                payload_metadata: None,
                created_at: 1,
            })
            .unwrap();
        assert_eq!(store.search("alice", 10).unwrap().len(), 1);

        purge_search_index(&store, &Detector::new().unwrap());
        assert!(store.search("alice", 10).unwrap().is_empty());
    }

    /// A detector update must be applied by the same startup helper Android
    /// calls. The fixture is a row written as ordinary by an older ruleset,
    /// with plaintext deliberately present in FTS.
    #[tokio::test]
    async fn the_embedded_startup_purge_removes_a_newly_sensitive_fts_row() {
        let (backend, _clip, _dir) = backend();
        let id = "indexed-before-detector-rule";
        let secret = "AKIAIOSFODNN7EXAMPLE";
        let (nonce, ciphertext) = copypaste_core::encrypt(
            secret.as_bytes(),
            &backend.inner.state.keyring.item_key(),
            id,
        )
        .unwrap();
        backend
            .inner
            .state
            .store
            .insert(copypaste_core::NewItem {
                id: id.into(),
                content_ciphertext: ciphertext,
                nonce,
                content_type: copypaste_ipc::content_type::TEXT.into(),
                content_hash: copypaste_core::compute_content_hash(secret.as_bytes()),
                is_sensitive: false,
                search_text: Some(secret.into()),
                app_bundle_id: None,
                payload_metadata: None,
                created_at: 1_700_000_000_000,
            })
            .unwrap();
        assert_eq!(
            backend.inner.state.store.search(secret, 10).unwrap().len(),
            1
        );

        assert_eq!(
            purge_search_index(&backend.inner.state.store, &backend.inner.state.detector),
            1
        );
        assert!(backend.search(secret, 10).await.unwrap().items.is_empty());
        assert!(
            backend.get(id).await.is_ok(),
            "the history item was deleted"
        );
    }
}
