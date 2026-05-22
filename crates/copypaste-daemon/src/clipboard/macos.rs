//! macOS clipboard monitor — NSPasteboard polling via objc2.
//!
//! Uses `changeCount` to detect changes without busy-waiting on the change
//! value; sleeps are handled by the caller (Tokio interval in daemon.rs).

use super::{ClipboardContent, ClipboardError, ClipboardMonitorTrait};

pub struct MacosClipboardMonitor {
    last_change_count: i64,
    max_text_bytes: u64,
}

impl MacosClipboardMonitor {
    pub fn new(max_text_bytes: u64) -> Self {
        Self { last_change_count: -1, max_text_bytes }
    }
}

impl ClipboardMonitorTrait for MacosClipboardMonitor {
    fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_starts_with_sentinel_count() {
        let m = MacosClipboardMonitor::new(1024);
        assert_eq!(m.last_change_count, -1);
    }
}
