//! Windows platform backend — stub for Phase 5a implementation.
//! Clipboard: Win32 AddClipboardFormatListener (WM_CLIPBOARDUPDATE)
//! Keystore: DPAPI CryptProtectData/CryptUnprotectData
//!
//! **FROZEN 2026-05-23.** Windows is out of scope for v0.3+ — see
//! `docs/adr/ADR-012-windows-frozen-homebrew-only.md`. Stubs retained
//! so the trait impls keep the workspace's `cfg(windows)` reachable
//! when (eventually) thawed; not compiled by any active CI target.
//! Do not delete.

use super::{ClipboardBackend, ClipboardEvent, KeystoreBackend};

/// Windows clipboard backend stub — implemented in Phase 5a.
pub struct WindowsClipboardBackend;

impl ClipboardBackend for WindowsClipboardBackend {
    fn next_change(&mut self) -> Option<ClipboardEvent> {
        unimplemented!("Windows clipboard backend — Phase 5a")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DpapiError {
    #[error("DPAPI operation failed: {0}")]
    Failed(String),
}

/// Windows DPAPI keystore stub — implemented in Phase 5a.
pub struct WindowsKeystoreBackend;

impl KeystoreBackend for WindowsKeystoreBackend {
    type Error = DpapiError;

    fn load_or_create(&self, _service: &str, _account: &str) -> Result<zeroize::Zeroizing<[u8; 32]>, DpapiError> {
        unimplemented!("Windows DPAPI keystore — Phase 5a")
    }

    fn store(&self, _service: &str, _account: &str, _secret: &[u8; 32]) -> Result<(), DpapiError> {
        unimplemented!()
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), DpapiError> {
        unimplemented!()
    }
}
