use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    Text(String),
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

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Content exceeds max size {max} bytes (got {actual})")]
    TooLarge { max: u64, actual: usize },
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
    pub fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};

            let (count, content) = unsafe {
                let pb = NSPasteboard::generalPasteboard();
                let count = pb.changeCount() as i64;
                let s = pb.stringForType(NSPasteboardTypeString);
                (count, s.map(|ns| ns.to_string()))
            };

            if count == self.last_change_count {
                return Ok(None);
            }
            self.last_change_count = count;

            if let Some(text) = content {
                if text.len() as u64 > self.max_text_bytes {
                    return Err(ClipboardError::TooLarge {
                        max: self.max_text_bytes,
                        actual: text.len(),
                    });
                }
                return Ok(Some(ClipboardContent::Text(text)));
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
    fn clipboard_monitor_new_starts_with_sentinel() {
        let m = ClipboardMonitor::new(1024);
        assert_eq!(m.last_change_count, -1);
    }

    #[test]
    fn too_large_error_on_oversized_content() {
        // Simulates what poll() would return if content exceeds limit
        let err = ClipboardError::TooLarge { max: 10, actual: 50 };
        assert!(err.to_string().contains("50"));
    }
}
