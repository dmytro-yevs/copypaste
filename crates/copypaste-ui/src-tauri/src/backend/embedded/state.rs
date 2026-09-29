//! Persistent state for the in-process backend.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;

use copypaste_core::{CryptoError, Keyring, Store, StoreError};
use copypaste_ipc::ErrorCode;

use crate::backend::{BackendError, Result};

use super::settings::EmbeddedSettings;

fn system_name() -> copypaste_core::device_name::SystemDeviceName {
    #[cfg(target_os = "android")]
    {
        crate::android_context::system_device_name()
    }
    #[cfg(not(target_os = "android"))]
    {
        copypaste_core::device_name::SystemDeviceName::current()
    }
}

pub(super) struct BackendState {
    pub(super) store: Store,
    // Behind an `Arc` because `copypaste_core::StoreSource` holds it
    // for as long as the peer listener runs.
    pub(super) keyring: Arc<Keyring>,
    /// This device's sync identity. The id is fixed; the display name is not.
    pub(super) device_id: String,
    device_name: RwLock<String>,
    /// Where the paired-device list lives. The `PeerStore` itself belongs to
    /// the node, which owns it by value.
    pub(super) peers_path: PathBuf,
    pub(super) settings: EmbeddedSettings,
}

impl BackendState {
    /// Open persistent state under the Android context's data directory.
    pub(super) fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| BackendError::internal(&format!("could not prepare storage: {e}")))?;

        // The same directory the database goes in, so the secret cannot end up
        // somewhere the history is not (security review F-11).
        let keyring = Keyring::load_or_create(data_dir).map_err(keyring_error)?;
        Self::open_with_keyring(data_dir, keyring)
    }

    /// Open the normal persistent state pipeline with an already-selected key.
    ///
    /// Production resolves its key through [`Self::open`]. Test fixtures inject
    /// a `Keyring::from_secret` so a temporary database cannot touch Keychain.
    pub(super) fn open_with_keyring(data_dir: &Path, keyring: Keyring) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| BackendError::internal(&format!("could not prepare storage: {e}")))?;

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
            .system_device_identity(&system_name())
            .map_err(|e| BackendError::internal(&format!("could not resolve this device: {e}")))?;

        Ok(Self {
            store,
            keyring: Arc::new(keyring),
            device_id: identity.device_id,
            device_name: RwLock::new(identity.device_name),
            // The name from the shared crate, as the daemon uses.
            peers_path: data_dir.join(copypaste_p2p::peers::DEFAULT_FILE_NAME),
            settings: EmbeddedSettings::open(data_dir.join("settings-v2.json")),
        })
    }

    pub(super) fn device_name(&self) -> String {
        self.device_name
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn refresh_system_name(&self) -> Result<bool> {
        self.refresh_name(&system_name())
    }

    pub(super) fn refresh_name(
        &self,
        system: &copypaste_core::device_name::SystemDeviceName,
    ) -> Result<bool> {
        let mut current = self.device_name.write().unwrap_or_else(|p| p.into_inner());
        let name = self
            .store
            .system_device_identity(system)
            .map_err(|_| BackendError::internal("the device name could not be saved"))?
            .device_name;
        let changed = *current != name;
        *current = name;
        Ok(changed)
    }

    pub(super) fn publish_device_name(&self, publish: impl FnOnce(&str)) {
        let name = self.device_name.read().unwrap_or_else(|p| p.into_inner());
        publish(&name);
    }

    pub(super) fn set_device_name(&self, name: &str) -> Result<String> {
        let mut current = self.device_name.write().unwrap_or_else(|p| p.into_inner());
        let name =
            self.store
                .set_device_name(&self.device_id, name)
                .map_err(|error| match error {
                    StoreError::InvalidDeviceName => {
                        BackendError::Invalid("A device name must contain visible text.")
                    }
                    _ => BackendError::internal("the device name could not be saved"),
                })?;
        *current = name.clone();
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
        | StoreError::InvalidKey
        | StoreError::IntegrityCheckFailed
        | StoreError::InvalidSchema
        | StoreError::NotFound
        | StoreError::InvalidCursor
        | StoreError::InvalidDeviceName => ErrorCode::Internal,
    };
    BackendError::from_code(Some(code), None, Some(&message))
}

#[cfg(test)]
mod tests {
    use super::super::tests::backend;
    use super::*;
    use crate::backend::Backend;
    use copypaste_core::Keyring;

    #[test]
    fn system_refresh_updates_the_cache_but_preserves_a_manual_name() {
        use copypaste_core::device_name::SystemDeviceName;
        let (backend, _, _dir) = backend();
        let state = &backend.inner.state;
        let system = SystemDeviceName::from_sources(Some("Phone after rename"), None);
        assert!(state.refresh_name(&system).unwrap());
        state.publish_device_name(|name| assert_eq!(name, "Phone after rename"));
        state.set_device_name("My phone").unwrap();
        assert!(!state.refresh_name(&system).unwrap());
        state.publish_device_name(|name| assert_eq!(name, "My phone"));
        assert_eq!(state.store.current_device_name().unwrap(), "My phone");
    }

    #[test]
    fn an_injected_key_reopens_its_store_and_wrong_keys_fail_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let first =
            BackendState::open_with_keyring(dir.path(), Keyring::from_secret(&[0x31; 32])).unwrap();
        let device_id = first.device_id.clone();
        drop(first);

        let reopened =
            BackendState::open_with_keyring(dir.path(), Keyring::from_secret(&[0x31; 32])).unwrap();
        assert_eq!(reopened.device_id, device_id);
        drop(reopened);

        assert!(
            BackendState::open_with_keyring(dir.path(), Keyring::from_secret(&[0x32; 32]),)
                .is_err()
        );
    }
}
