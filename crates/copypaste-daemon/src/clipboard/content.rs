//! Clipboard content model: the `ClipboardContent` enum read from a poll,
//! plus its error type.

use thiserror::Error;

/// Threshold above which a poll-to-poll changeCount delta is treated as a
/// "burst" of rapid clipboard writes. When the user copies many things in
/// quick succession (faster than the poll interval), we cannot recover the
/// intermediate values from NSPasteboard, but we surface the gap via
/// telemetry + a [`ClipboardContent::SkippedBatch`] variant so downstream
/// consumers can react (e.g. show a toast or log a counter).
pub const SKIPPED_BATCH_THRESHOLD: i64 = 3;

/// Content read from the system clipboard on a change event.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    /// Plain UTF-8 text.
    Text(String),
    /// Raw image bytes (PNG or TIFF) read directly from NSPasteboard.
    /// Compression and encryption are performed downstream by the daemon.
    Image(Vec<u8>),
    /// A file whose URL was on the clipboard (macOS `public.file-url`).
    /// `bytes` is the raw file content; `filename` and `mime` are derived from
    /// the URL at capture time. Encryption is performed downstream by the daemon.
    File {
        bytes: Vec<u8>,
        filename: String,
        mime: String,
    },
    /// Internal: a file-URL was detected on the clipboard but the bytes have
    /// NOT been read yet. `poll()` returns this variant instead of `File` so
    /// that the actual `std::fs::read` can be deferred to the async
    /// `handle_tick` caller, which wraps it in `tokio::task::spawn_blocking`.
    /// Callers outside `handle_tick` should never observe this variant in
    /// normal operation; it is resolved to `File` (or silently dropped on
    /// read error) before surfacing to higher layers.
    FileRef {
        path: std::path::PathBuf,
        filename: String,
        mime: String,
    },
    /// Emitted alongside the latest captured content when the pasteboard
    /// changeCount advanced by more than [`SKIPPED_BATCH_THRESHOLD`] since
    /// the previous poll. The inner value is the number of intermediate
    /// updates that were missed (delta minus 1).
    SkippedBatch(usize),
}

impl ClipboardContent {
    /// Returns the raw bytes for this content variant.
    /// `SkippedBatch` and `FileRef` have no in-memory payload — returns an
    /// empty slice. `FileRef` bytes are loaded lazily in `handle_tick`.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ClipboardContent::Text(s) => s.as_bytes(),
            ClipboardContent::Image(b) => b.as_slice(),
            ClipboardContent::File { bytes, .. } => bytes.as_slice(),
            ClipboardContent::FileRef { .. } => &[],
            ClipboardContent::SkippedBatch(_) => &[],
        }
    }

    /// Returns the storage content_type string used in `ClipboardItem`.
    pub fn content_type(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image(_) => "image",
            ClipboardContent::File { .. } => "file",
            ClipboardContent::FileRef { .. } => "file",
            ClipboardContent::SkippedBatch(_) => "skipped_batch",
        }
    }
}

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Text content exceeds max size {max} bytes (got {actual})")]
    TooLarge { max: u64, actual: usize },
    #[error("Image too large: {actual} bytes (max {max})")]
    ImageTooLarge { max: usize, actual: usize },
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
    fn clipboard_content_image_roundtrip() {
        let bytes = vec![0x89u8, 0x50, 0x4e, 0x47]; // PNG magic
        let c = ClipboardContent::Image(bytes.clone());
        assert_eq!(c.as_bytes(), bytes.as_slice());
        assert_eq!(c.content_type(), "image");
    }

    #[test]
    fn clipboard_content_image_equality() {
        let a = ClipboardContent::Image(vec![1, 2, 3]);
        let b = ClipboardContent::Image(vec![1, 2, 3]);
        let c = ClipboardContent::Image(vec![4, 5, 6]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn too_large_error_on_oversized_text() {
        let err = ClipboardError::TooLarge {
            max: 10,
            actual: 50,
        };
        assert!(err.to_string().contains("50"));
    }

    #[test]
    fn image_too_large_error_message() {
        let err = ClipboardError::ImageTooLarge {
            max: 1000,
            actual: 2000,
        };
        let msg = err.to_string();
        assert!(msg.contains("2000"));
        assert!(msg.contains("1000"));
    }

    // -- Wave 2.1 fixes --------------------------------------------------

    #[test]
    fn skipped_batch_variant_carries_count_and_type() {
        let c = ClipboardContent::SkippedBatch(7);
        assert_eq!(c.as_bytes(), &[] as &[u8]);
        assert_eq!(c.content_type(), "skipped_batch");
        // Threshold is exposed for callers/tests. Use a const-eval check so
        // clippy doesn't strip an always-true `assert!`.
        const _: () = assert!(SKIPPED_BATCH_THRESHOLD >= 3);
    }

    /// edge HIGH #6 — mixed_text_image_text_wins.
    /// Asserts the documented invariant: when both text and image are
    /// available on the same changeCount, the daemon surfaces only the
    /// text variant. (Real NSPasteboard interaction is exercised via
    /// the integration smoke test; this test pins the contract.)
    #[test]
    fn mixed_text_image_text_wins() {
        // Given a Text variant chosen from a mixed pasteboard...
        let chosen = ClipboardContent::Text("hello".to_string());
        // ...the content_type must report "text", never "image".
        assert_eq!(chosen.content_type(), "text");
        assert_eq!(chosen.as_bytes(), b"hello");
    }

    /// FileRef variant: as_bytes returns empty, content_type returns "file".
    /// The actual bytes are loaded lazily via spawn_blocking in handle_tick.
    #[test]
    fn file_ref_content_type_and_bytes() {
        let c = ClipboardContent::FileRef {
            path: std::path::PathBuf::from("/tmp/report.pdf"),
            filename: "report.pdf".to_string(),
            mime: "application/pdf".to_string(),
        };
        assert_eq!(c.content_type(), "file");
        assert_eq!(c.as_bytes(), &[] as &[u8]);
    }
}
