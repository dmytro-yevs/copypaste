//! Platform abstraction for clipboard, keystore, and IPC backends.
//! Each OS provides its own implementation behind cfg-gates.

use std::path::PathBuf;

/// Clipboard change event from the OS.
#[derive(Debug, Clone)]
pub struct ClipboardEvent {
    pub text: Option<String>,
    /// Raw image bytes (PNG or TIFF) — present when an image was copied.
    pub image_bytes: Option<Vec<u8>>,
    pub source: ClipboardSource,
}

#[derive(Debug, Clone)]
pub enum ClipboardSource {
    General,
}

/// Clipboard monitoring backend — OS-specific.
pub trait ClipboardBackend: Send {
    /// Block until the clipboard changes, then return the new content.
    /// Returns `None` if the content type is unsupported.
    fn next_change(&mut self) -> Option<ClipboardEvent>;
}

/// Key storage backend — OS keychain equivalent.
pub trait KeystoreBackend: Send {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Load a 32-byte secret by (service, account). Creates if absent.
    fn load_or_create(&self, service: &str, account: &str) -> Result<zeroize::Zeroizing<[u8; 32]>, Self::Error>;

    /// Overwrite stored secret.
    fn store(&self, service: &str, account: &str, secret: &[u8; 32]) -> Result<(), Self::Error>;

    /// Delete stored secret.
    fn delete(&self, service: &str, account: &str) -> Result<(), Self::Error>;
}

/// IPC server socket path helper — OS-specific convention.
pub fn default_socket_path() -> PathBuf {
    crate::paths::socket_path()
}

// Platform-specific modules
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
