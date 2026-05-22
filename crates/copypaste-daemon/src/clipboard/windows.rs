//! Windows clipboard monitor — WM_CLIPBOARDUPDATE via hidden Win32 window.
//!
//! # Architecture
//!
//! A dedicated OS thread creates a hidden HWND and calls
//! `AddClipboardFormatListener(hwnd)`.  The Win32 message loop runs on that
//! thread; on every `WM_CLIPBOARDUPDATE` it reads the clipboard and sends the
//! result through a `std::sync::mpsc::Sender<ClipboardContent>`.
//!
//! The Tokio side holds the `Receiver` and drains it on each poll() call.
//!
//! # TODO (Phase 2)
//!
//! - [ ] Call `CreateWindowExW` with a unique WNDCLASS name ("CopyPasteClip")
//! - [ ] Register `WndProc` handling `WM_CLIPBOARDUPDATE` and `WM_DESTROY`
//! - [ ] Call `AddClipboardFormatListener(hwnd)` after window creation
//! - [ ] In WndProc on `WM_CLIPBOARDUPDATE`: call `read_clipboard_text()` and
//!       send over `Sender`
//! - [ ] Implement `read_clipboard_text()`:
//!         `IsClipboardFormatAvailable(CF_UNICODETEXT)` → `OpenClipboard(None)`
//!         → `GetClipboardData(CF_UNICODETEXT)` → `GlobalLock` → UTF-16 decode
//!         → `GlobalUnlock` → `CloseClipboard()`
//! - [ ] Retry `OpenClipboard` up to 5× with 10ms back-off if another process
//!       holds the clipboard
//! - [ ] On drop: `PostMessageW(hwnd, WM_DESTROY, 0, 0)` to exit message loop;
//!       call `RemoveClipboardFormatListener(hwnd)` before `DestroyWindow`
//! - [ ] Compile-guard all Win32 calls behind `#[cfg(target_os = "windows")]`

use super::{ClipboardContent, ClipboardError, ClipboardMonitorTrait};
use std::sync::mpsc;

pub struct WindowsClipboardMonitor {
    max_text_bytes: u64,
    receiver: mpsc::Receiver<Result<ClipboardContent, ClipboardError>>,
    // TODO(Phase 2): store HWND handle for clean shutdown
    // hwnd: windows::Win32::Foundation::HWND,
    _thread_handle: std::thread::JoinHandle<()>,
}

impl WindowsClipboardMonitor {
    /// Spawn the Win32 message-loop thread and return a monitor that receives
    /// clipboard events from it.
    pub fn new(max_text_bytes: u64) -> Self {
        let (tx, rx) = mpsc::channel();

        // TODO(Phase 2): replace this stub thread with the real Win32 loop.
        // The stub sends nothing and just parks, so `poll()` always returns Ok(None).
        let _thread_handle = std::thread::spawn(move || {
            // STUB: Phase 2 will:
            //   1. Register WNDCLASS with WndProc
            //   2. CreateWindowExW (hidden, WS_OVERLAPPEDWINDOW | WS_MINIMIZE)
            //   3. AddClipboardFormatListener(hwnd)
            //   4. Loop: GetMessageW(&msg, None, 0, 0) → TranslateMessage → DispatchMessageW
            //   5. On WM_CLIPBOARDUPDATE → read_clipboard_text() → tx.send(...)
            //   6. On WM_DESTROY → break
            let _ = tx; // keep tx alive; drop = receiver gets disconnected on shutdown
            std::thread::park(); // placeholder until Phase 2
        });

        Self { max_text_bytes, receiver: rx, _thread_handle }
    }
}

impl ClipboardMonitorTrait for WindowsClipboardMonitor {
    /// Non-blocking drain of events from the Win32 thread.
    ///
    /// Returns the most recent clipboard change, or `Ok(None)` if no change
    /// occurred since the last poll.
    fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
        let mut last: Option<Result<ClipboardContent, ClipboardError>> = None;

        // Drain all queued events; keep only the most recent.
        loop {
            match self.receiver.try_recv() {
                Ok(event) => last = Some(event),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        match last {
            None => Ok(None),
            Some(Ok(content)) => {
                let len = content.as_bytes().len();
                if len as u64 > self.max_text_bytes {
                    return Err(ClipboardError::TooLarge {
                        max: self.max_text_bytes,
                        actual: len,
                    });
                }
                Ok(Some(content))
            }
            Some(Err(e)) => Err(e),
        }
    }
}

// TODO(Phase 2): implement `read_clipboard_text` as a free fn called from WndProc.
// Signature will be:
//   fn read_clipboard_text() -> Option<String>
// Steps:
//   1. IsClipboardFormatAvailable(CF_UNICODETEXT) → return None if false
//   2. OpenClipboard(None) with retry loop (up to 5 attempts, 10ms sleep)
//   3. GetClipboardData(CF_UNICODETEXT) → HANDLE
//   4. GlobalLock(handle) → *const u16
//   5. Collect UTF-16 chars until null terminator
//   6. String::from_utf16_lossy(&chars)
//   7. GlobalUnlock(handle)
//   8. CloseClipboard()

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_monitor_construction_succeeds() {
        // Smoke test: constructor should not panic on any platform (stub implementation).
        let mut monitor = WindowsClipboardMonitor::new(1024 * 1024);
        // Stub thread never sends, so poll returns None.
        assert!(matches!(monitor.poll(), Ok(None)));
    }

    #[test]
    fn too_large_error_via_channel() {
        let (tx, rx) = std::sync::mpsc::channel();
        // Simulate the Win32 thread sending a large item.
        tx.send(Ok(ClipboardContent::Text("x".repeat(10)))).unwrap();
        drop(tx);

        let monitor = WindowsClipboardMonitor {
            max_text_bytes: 5,
            receiver: rx,
            _thread_handle: std::thread::spawn(|| {}),
        };
        // Need to manually call the drain logic — mirror what poll() does.
        // This tests the size-check branch without requiring Win32.
        let mut m = monitor;
        let result = m.poll();
        assert!(matches!(result, Err(ClipboardError::TooLarge { .. })));
    }
}
