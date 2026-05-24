//! macOS platform backend — wraps existing clipboard.rs + keychain.rs.
//! Full implementation lives in those modules; this re-exports the trait impls.

use super::{ClipboardBackend, ClipboardEvent, ClipboardSource, KeystoreBackend};
use crate::clipboard::{ClipboardContent, ClipboardMonitor};
use crate::keychain::{self, KeychainError};

/// macOS clipboard backend wrapping NSPasteboard polling.
pub struct MacosClipboardBackend {
    monitor: ClipboardMonitor,
    poll_interval_ms: u64,
}

impl MacosClipboardBackend {
    pub fn new(max_text_bytes: u64, poll_interval_ms: u64) -> Self {
        Self {
            monitor: ClipboardMonitor::new(max_text_bytes),
            poll_interval_ms,
        }
    }
}

impl ClipboardBackend for MacosClipboardBackend {
    fn next_change(&mut self) -> Option<ClipboardEvent> {
        std::thread::sleep(std::time::Duration::from_millis(self.poll_interval_ms));
        match self.monitor.poll() {
            Ok(Some(ClipboardContent::Text(text))) => Some(ClipboardEvent {
                text: Some(text),
                image_bytes: None,
                source: ClipboardSource::General,
            }),
            Ok(Some(ClipboardContent::Image(bytes))) => Some(ClipboardEvent {
                text: None,
                image_bytes: Some(bytes),
                source: ClipboardSource::General,
            }),
            _ => None,
        }
    }
}

/// macOS keystore backend — macOS Keychain via security-framework.
pub struct MacosKeystoreBackend;

impl KeystoreBackend for MacosKeystoreBackend {
    type Error = KeychainError;

    fn load_or_create(
        &self,
        _service: &str,
        _account: &str,
    ) -> Result<zeroize::Zeroizing<[u8; 32]>, Self::Error> {
        keychain::load_or_create().map(|kp| kp.secret_key_bytes_zeroizing())
    }

    fn store(&self, _service: &str, _account: &str, _secret: &[u8; 32]) -> Result<(), Self::Error> {
        // Keychain stores on load_or_create; explicit store not yet needed
        Ok(())
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), Self::Error> {
        #[cfg(target_os = "macos")]
        return keychain::delete_stored();
        #[cfg(not(target_os = "macos"))]
        Ok(())
    }
}
