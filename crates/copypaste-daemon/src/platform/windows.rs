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
        // Windows is frozen (ADR-012). The trait returns Option, not Result, so
        // a graceful error return is not possible here. Returning None signals
        // "no event available" and avoids a panic; the caller's polling loop
        // will simply see no clipboard changes on Windows rather than crashing.
        None
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

    fn load_or_create(
        &self,
        _service: &str,
        _account: &str,
    ) -> Result<zeroize::Zeroizing<[u8; 32]>, DpapiError> {
        // Windows is frozen (ADR-012) — DPAPI implementation is not available.
        Err(DpapiError::Failed(
            "Windows is not supported (frozen, ADR-012)".to_string(),
        ))
    }

    fn store(&self, _service: &str, _account: &str, _secret: &[u8; 32]) -> Result<(), DpapiError> {
        // Windows is frozen (ADR-012) — DPAPI implementation is not available.
        Err(DpapiError::Failed(
            "Windows is not supported (frozen, ADR-012)".to_string(),
        ))
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), DpapiError> {
        // Windows is frozen (ADR-012) — DPAPI implementation is not available.
        Err(DpapiError::Failed(
            "Windows is not supported (frozen, ADR-012)".to_string(),
        ))
    }
}
