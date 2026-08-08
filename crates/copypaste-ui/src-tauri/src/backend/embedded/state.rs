//! Persistent state and startup repair for the in-process backend.
//!
//! The v2 store, its keys, the device identity and the detector are opened as
//! one unit. The search-index purge is best-effort: losing a stale FTS row is
//! better than blocking access to encrypted history when SQLite cannot read
//! the index.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;

use copypaste_core::{purge_indexed_secrets, CryptoError, Detector, Keyring, Store, StoreError};
use copypaste_ipc::ErrorCode;

use crate::backend::{BackendError, Result};

use super::settings::EmbeddedSettings;

/// First-run name on Android, where the hostname is only `localhost`.
const DEVICE_NAME_HINT: &str = "CopyPaste phone";

pub(super) struct BackendState {
    pub(super) store: Store,
    // Both are behind an `Arc` because `copypaste_core::StoreSource` holds them
    // for as long as the peer listener runs.
    pub(super) keyring: Arc<Keyring>,
    pub(super) detector: Arc<Detector>,
    /// This device's sync identity. The id is fixed; the display name is not.
    pub(super) device_id: String,
    device_name: RwLock<String>,
    /// Where the paired-device list lives. The `PeerStore` itself belongs to
    /// the node, which owns it by value.
    pub(super) peers_path: PathBuf,
    pub(super) settings: EmbeddedSettings,
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
        let keyring = Keyring::load_or_create(data_dir).map_err(keyring_error)?;

        // The database filename comes from the shared crate.
        let db_path = data_dir.join(
            copypaste_ipc::database_path()
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("copypaste-v2.db")),
        );
        let store = Store::open(&db_path, &keyring.db_key()).map_err(store_open_error)?;

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
            keyring: Arc::new(keyring),
            detector: Arc::new(detector),
            device_id: identity.device_id,
            device_name: RwLock::new(identity.device_name),
            // The name from the shared crate, as the daemon uses.
            peers_path: data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
            settings: EmbeddedSettings::open(data_dir.join("settings-v2.json")),
            index_purged,
        })
    }

    pub(super) fn device_name(&self) -> String {
        self.device_name
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn set_device_name(&self, name: &str) -> Result<String> {
        let name =
            self.store
                .set_device_name(&self.device_id, name)
                .map_err(|error| match error {
                    StoreError::InvalidDeviceName => {
                        BackendError::Invalid("A device name must contain visible text.")
                    }
                    _ => BackendError::internal("the device name could not be saved"),
                })?;
        *self
            .device_name
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = name.clone();
        Ok(name)
    }
}

fn keyring_error(error: CryptoError) -> BackendError {
    let message = format!("could not open the keystore: {error}");
    let code = match error {
        CryptoError::KeystoreUnavailable(_) => ErrorCode::KeyLocked,
        CryptoError::KeystoreEntryUnusable(_) => ErrorCode::KeyUnusable,
        CryptoError::AuthFailed | CryptoError::InvalidNonce | CryptoError::Internal(_) => {
            ErrorCode::Internal
        }
    };
    BackendError::from_code(Some(code), None, Some(&message))
}

fn store_open_error(error: StoreError) -> BackendError {
    let message = format!("could not open history: {error}");
    let code = match error {
        StoreError::File(_)
        | StoreError::Sqlite(_)
        | StoreError::Pool(_)
        | StoreError::Migration(_)
        | StoreError::InvalidKey
        | StoreError::IntegrityCheckFailed
        | StoreError::InvalidSchema
        | StoreError::NotFound
        | StoreError::InvalidCursor
        | StoreError::InvalidDeviceName => ErrorCode::Internal,
    };
    BackendError::from_code(Some(code), None, Some(&message))
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

    fn index_row(store: &Store, id: &str, text: &str) {
        store
            .insert(copypaste_core::NewItem {
                id: id.to_string(),
                content_ciphertext: Vec::new(),
                nonce: Vec::new(),
                content_type: "text/plain".to_string(),
                content_hash: id.to_string(),
                is_sensitive: false,
                search_text: Some(text.to_string()),
                app_bundle_id: None,
                app_name: None,
                payload_metadata: None,
                created_at: 1,
            })
            .unwrap();
    }

    /// The purge takes the high-confidence band and stops there.
    ///
    /// Both directions, because only the pair pins the floor: purging what is
    /// merely flagged costs the user every search that would have found it,
    /// and `wipe.rs` uses this same email fixture as its canonical
    /// below-the-floor case.
    #[test]
    fn the_embedded_open_purge_removes_secrets_and_leaves_the_merely_flagged() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("copypaste-v2.db"), &[7; 32]).unwrap();
        index_row(&store, "above-the-floor", "AKIAIOSFODNN7EXAMPLE is the key");
        index_row(
            &store,
            "below-the-floor",
            "mail alice.smith@example.com about it",
        );
        assert_eq!(store.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().len(), 1);
        assert_eq!(store.search("alice", 10).unwrap().len(), 1);

        assert_eq!(purge_search_index(&store, &Detector::new().unwrap()), 1);
        assert!(store.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().is_empty());
        assert_eq!(
            store.search("alice", 10).unwrap().len(),
            1,
            "a flagged-only row must stay searchable (CLAUDE.md rule 4)"
        );
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
                app_name: None,
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
