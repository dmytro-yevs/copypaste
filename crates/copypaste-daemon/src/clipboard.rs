//! Clipboard monitor: polls NSPasteboard (macOS) for text and image changes.

use thiserror::Error;

/// Content read from the system clipboard on a change event.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    /// Plain UTF-8 text.
    Text(String),
    /// Raw image bytes (PNG or TIFF) read directly from NSPasteboard.
    /// Compression and encryption are performed downstream by the daemon.
    Image(Vec<u8>),
}

impl ClipboardContent {
    /// Returns the raw bytes for this content variant.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ClipboardContent::Text(s) => s.as_bytes(),
            ClipboardContent::Image(b) => b.as_slice(),
        }
    }

    /// Returns the storage content_type string used in `ClipboardItem`.
    pub fn content_type(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image(_) => "image",
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

pub struct ClipboardMonitor {
    last_change_count: i64,
    max_text_bytes: u64,
}

impl ClipboardMonitor {
    pub fn new(max_text_bytes: u64) -> Self {
        Self { last_change_count: -1, max_text_bytes }
    }

    /// Poll for new clipboard content. Returns `Some` only if the pasteboard changed.
    ///
    /// Priority: text > PNG > TIFF.  Image data is returned as raw bytes for
    /// downstream compression + encryption.
    pub fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
        #[cfg(target_os = "macos")]
        {
            use copypaste_core::MAX_IMAGE_BYTES;
            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
            use objc2_foundation::NSData;

            let (count, text, image_bytes) = unsafe {
                let pb = NSPasteboard::generalPasteboard();
                let count = pb.changeCount() as i64;

                // Text
                let text = pb
                    .stringForType(NSPasteboardTypeString)
                    .map(|ns| ns.to_string());

                // Image — try PNG first, then TIFF
                let image_bytes: Option<Vec<u8>> = if text.is_none() {
                    // NSPasteboardTypePNG is "public.png"
                    let png_type = objc2_foundation::NSString::from_str("public.png");
                    let tiff_type = objc2_foundation::NSString::from_str("public.tiff");

                    // Try PNG
                    let png_data = pb.dataForType(&png_type);
                    if let Some(ref d) = png_data {
                        Some(d.bytes().to_vec())
                    } else {
                        // Try TIFF
                        let tiff_data = pb.dataForType(&tiff_type);
                        tiff_data.as_deref().map(|d: &NSData| d.bytes().to_vec())
                    }
                } else {
                    None
                };

                (count, text, image_bytes)
            };

            if count == self.last_change_count {
                return Ok(None);
            }
            self.last_change_count = count;

            if let Some(text) = text {
                if text.len() as u64 > self.max_text_bytes {
                    return Err(ClipboardError::TooLarge {
                        max: self.max_text_bytes,
                        actual: text.len(),
                    });
                }
                return Ok(Some(ClipboardContent::Text(text)));
            }

            if let Some(bytes) = image_bytes {
                if bytes.len() > MAX_IMAGE_BYTES {
                    return Err(ClipboardError::ImageTooLarge {
                        max: MAX_IMAGE_BYTES,
                        actual: bytes.len(),
                    });
                }
                return Ok(Some(ClipboardContent::Image(bytes)));
            }

            Ok(None)
        }
        #[cfg(not(target_os = "macos"))]
        Ok(None)
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
    fn clipboard_monitor_new_starts_with_sentinel() {
        let m = ClipboardMonitor::new(1024);
        assert_eq!(m.last_change_count, -1);
    }

    #[test]
    fn too_large_error_on_oversized_text() {
        let err = ClipboardError::TooLarge { max: 10, actual: 50 };
        assert!(err.to_string().contains("50"));
    }

    #[test]
    fn image_too_large_error_message() {
        let err = ClipboardError::ImageTooLarge { max: 1000, actual: 2000 };
        let msg = err.to_string();
        assert!(msg.contains("2000"));
        assert!(msg.contains("1000"));
    }
}
