//! Linux platform backend — stub for Phase 5b.
//! Clipboard: x11-clipboard or wl-clipboard-rs (feature flags)
//! Keystore: secret-service (GNOME Keyring / KWallet)

use super::{ClipboardBackend, ClipboardEvent, KeystoreBackend};

/// Linux clipboard backend stub — implemented in Phase 5b.
pub struct LinuxClipboardBackend;

impl ClipboardBackend for LinuxClipboardBackend {
    fn next_change(&mut self) -> Option<ClipboardEvent> {
        unimplemented!("Linux clipboard backend — Phase 5b")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretServiceError {
    #[error("Secret Service error: {0}")]
    Error(String),
}

/// Linux Secret Service keystore stub — implemented in Phase 5b.
pub struct LinuxKeystoreBackend;

impl KeystoreBackend for LinuxKeystoreBackend {
    type Error = SecretServiceError;

    fn load_or_create(&self, _s: &str, _a: &str) -> Result<[u8; 32], SecretServiceError> {
        unimplemented!("Linux Secret Service — Phase 5b")
    }

    fn store(&self, _s: &str, _a: &str, _sec: &[u8; 32]) -> Result<(), SecretServiceError> {
        unimplemented!()
    }

    fn delete(&self, _s: &str, _a: &str) -> Result<(), SecretServiceError> {
        unimplemented!()
    }
}
