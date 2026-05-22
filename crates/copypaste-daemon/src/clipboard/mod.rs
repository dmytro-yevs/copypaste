//! Clipboard monitoring — cross-platform abstraction.
//!
//! The `ClipboardMonitor` trait defines the polling interface.
//! Platform implementations live in `macos.rs` and `windows.rs`.

use thiserror::Error;

/// Content captured from the clipboard.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    Text(String),
    // Image variant reserved for Phase 4 (CF_DIB / NSBitmapImageRep)
}

impl ClipboardContent {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ClipboardContent::Text(s) => s.as_bytes(),
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
        }
    }
}

/// Errors that can occur during clipboard polling.
#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Content exceeds max size {max} bytes (got {actual})")]
    TooLarge { max: u64, actual: usize },
    #[error("Clipboard access failed: {0}")]
    AccessError(String),
}

/// Cross-platform clipboard monitor.
///
/// Implementations must be constructable via `new(max_text_bytes)` and
/// support synchronous `poll()` for use in a Tokio `spawn_blocking` task.
pub trait ClipboardMonitorTrait: Send + 'static {
    /// Poll for a clipboard change. Returns `Some` only if the clipboard
    /// changed since the last call. Returns `None` if unchanged or the new
    /// content is an unsupported format.
    fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError>;
}

// Re-export the platform-specific concrete type as `ClipboardMonitor`.
#[cfg(target_os = "macos")]
pub use macos::MacosClipboardMonitor as ClipboardMonitor;

#[cfg(target_os = "windows")]
pub use windows::WindowsClipboardMonitor as ClipboardMonitor;

// Fallback for Linux / CI builds — returns None on every poll.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use fallback::FallbackClipboardMonitor as ClipboardMonitor;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod fallback;

// Keep the fallback module always compiled so non-platform CI passes.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod fallback {
    use super::{ClipboardContent, ClipboardError, ClipboardMonitorTrait};

    pub struct FallbackClipboardMonitor {
        pub max_text_bytes: u64,
    }

    impl FallbackClipboardMonitor {
        pub fn new(max_text_bytes: u64) -> Self {
            Self { max_text_bytes }
        }
    }

    impl ClipboardMonitorTrait for FallbackClipboardMonitor {
        fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
            // No clipboard support on this platform.
            let _ = self.max_text_bytes;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_content_text_roundtrip() {
        let c = ClipboardContent::Text("hello".to_string());
        assert_eq!(c.as_bytes(), b"hello");
        assert_eq!(c.content_type(), "text");
    }

    #[test]
    fn too_large_error_display() {
        let err = ClipboardError::TooLarge { max: 10, actual: 50 };
        assert!(err.to_string().contains("50"));
    }
}
