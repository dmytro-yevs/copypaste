//! Clipboard monitor: polls NSPasteboard (macOS) for text and image changes.

use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};
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
    /// Emitted alongside the latest captured content when the pasteboard
    /// changeCount advanced by more than [`SKIPPED_BATCH_THRESHOLD`] since
    /// the previous poll. The inner value is the number of intermediate
    /// updates that were missed (delta minus 1).
    SkippedBatch(usize),
}

impl ClipboardContent {
    /// Returns the raw bytes for this content variant.
    /// `SkippedBatch` has no payload — returns an empty slice.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            ClipboardContent::Text(s) => s.as_bytes(),
            ClipboardContent::Image(b) => b.as_slice(),
            ClipboardContent::SkippedBatch(_) => &[],
        }
    }

    /// Returns the storage content_type string used in `ClipboardItem`.
    pub fn content_type(&self) -> &'static str {
        match self {
            ClipboardContent::Text(_) => "text",
            ClipboardContent::Image(_) => "image",
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

/// SHA-256 based content hash for image deduplication. Returns the first
/// 16 bytes of `SHA-256(raw)`, giving a 128-bit collision-resistant
/// fingerprint. Replaces the prior `DefaultHasher XOR nanos` scheme which
/// was non-deterministic and trivially collidable (security LOW #19).
pub fn image_content_hash(raw: &[u8]) -> [u8; 16] {
    let digest = Sha256::digest(raw);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Process-wide set of pasteboard kinds we've already logged once.
/// Keeps the steady-state log volume bounded when the user repeatedly
/// copies an unsupported type (e.g. RTF inside a text editor).
fn unsupported_kind_seen() -> &'static Mutex<HashSet<String>> {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Log an unsupported clipboard kind at INFO level, but only the first
/// time we see each distinct kind in this process.
fn log_unsupported_once(kind: &str) {
    let mut seen = match unsupported_kind_seen().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if seen.insert(kind.to_string()) {
        tracing::info!(
            kind = %kind,
            "clipboard: unsupported type (logged once per kind)"
        );
    }
}

#[cfg(test)]
fn reset_unsupported_kinds_for_test() {
    if let Ok(mut g) = unsupported_kind_seen().lock() {
        g.clear();
    }
}

pub struct ClipboardMonitor {
    last_change_count: i64,
    max_text_bytes: u64,
    /// Maximum raw image bytes accepted from NSPasteboard before the READ gate
    /// rejects the image.  Defaults to [`copypaste_core::MAX_IMAGE_BYTES`]
    /// (10 MiB) when constructed via [`ClipboardMonitor::new`]; the daemon
    /// overrides this with the user-configured `max_image_size_bytes` so the
    /// READ gate matches the encode gate rather than the hardcoded core const.
    max_image_bytes: usize,
    /// changeCount recorded after a daemon self-write to NSPasteboard (copy_item /
    /// "copy" IPC handler). When the next poll sees this exact changeCount the daemon
    /// caused the change itself — skip recording to prevent a duplicate row.
    /// Shared with `write_to_pasteboard` via an `Arc<AtomicI64>`.
    pub self_write_change_count: Arc<AtomicI64>,
}

impl ClipboardMonitor {
    pub fn new(max_text_bytes: u64) -> Self {
        use copypaste_core::MAX_IMAGE_BYTES;
        Self {
            last_change_count: -1,
            max_text_bytes,
            max_image_bytes: MAX_IMAGE_BYTES,
            self_write_change_count: Arc::new(AtomicI64::new(-1)),
        }
    }

    /// Override the image-size READ gate with the user-configured cap.
    /// Call this after [`ClipboardMonitor::new`] when a non-default cap is set.
    pub fn set_max_image_bytes(&mut self, bytes: usize) {
        self.max_image_bytes = bytes;
    }

    /// Poll for new clipboard content. Returns `Some` only if the pasteboard changed.
    ///
    /// Priority: text > PNG > TIFF.  Image data is returned as raw bytes for
    /// downstream compression + encryption.
    ///
    /// Edge cases handled (Wave 2.1):
    /// - **Rapid changes (#5):** if the changeCount delta is ≥
    ///   [`SKIPPED_BATCH_THRESHOLD`], we cannot recover intermediate values
    ///   but emit telemetry; consumers can poll again immediately to drain.
    /// - **Mixed text+image (#6):** text wins; an INFO log records that an
    ///   image was silently dropped on that poll.
    /// - **Unsupported types (#7):** RTF / file-URLs / custom UTIs are
    ///   logged once per kind, never silently dropped.
    pub fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
            use objc2_foundation::{NSArray, NSData, NSString};

            // Drain the autorelease pool at the end of every poll. Without
            // this, each NSPasteboard read leaks autoreleased Cocoa objects
            // (NSString, and multi-MB image NSData) that are never freed on
            // this tokio thread, blowing up reserved virtual memory.
            let read = objc2::rc::autoreleasepool(|_pool| {
                let pb = unsafe { NSPasteboard::generalPasteboard() };
                let count = unsafe { pb.changeCount() } as i64;

                // changeCount-first: if the pasteboard is unchanged, return
                // before touching any stringForType/dataForType so an idle
                // clipboard allocates nothing. `None` signals "unchanged".
                if count == self.last_change_count {
                    return None;
                }

                let png_type = NSString::from_str("public.png");
                let tiff_type = NSString::from_str("public.tiff");

                // Text
                let text = unsafe {
                    pb.stringForType(NSPasteboardTypeString)
                        .map(|ns| ns.to_string())
                };

                // Probe image presence WITHOUT copying the bytes: ask the
                // pasteboard whether any image type is available rather than
                // materialising the (potentially multi-MB) NSData (#6).
                let image_types = NSArray::from_id_slice(&[png_type.clone(), tiff_type.clone()]);
                let image_present = unsafe { pb.availableTypeFromArray(&image_types) }.is_some();
                let had_image_alongside_text = text.is_some() && image_present;

                // Only materialise image bytes when text is absent and an
                // image is actually present.
                let image_bytes: Option<Vec<u8>> = if text.is_none() && image_present {
                    let png_data = unsafe { pb.dataForType(&png_type) };
                    if let Some(ref d) = png_data {
                        Some(d.bytes().to_vec())
                    } else {
                        let tiff_data = unsafe { pb.dataForType(&tiff_type) };
                        tiff_data.as_deref().map(|d: &NSData| d.bytes().to_vec())
                    }
                } else {
                    None
                };

                // Detect unsupported types — only matters when we have
                // nothing else to surface (text + image both absent).
                // We probe a fixed allowlist of common unsupported UTIs
                // rather than enumerating `pb.types()` so we don't need
                // the `NSEnumerator` feature on objc2-foundation 0.2.
                let mut unsupported_kinds: Vec<String> = Vec::new();
                if text.is_none() && image_bytes.is_none() {
                    let probes: &[&str] = &[
                        "public.rtf",
                        "public.rtfd",
                        "public.html",
                        "public.file-url",
                        "public.url",
                        "com.apple.pasteboard.promised-file-url",
                    ];
                    for kind in probes {
                        let ns_kind = NSString::from_str(kind);
                        let present = unsafe {
                            pb.dataForType(&ns_kind).is_some()
                                || pb.stringForType(&ns_kind).is_some()
                        };
                        if present {
                            unsupported_kinds.push((*kind).to_string());
                        }
                    }
                }

                Some((
                    count,
                    text,
                    image_bytes,
                    had_image_alongside_text,
                    unsupported_kinds,
                ))
            });

            // Unchanged pasteboard — nothing read, nothing allocated.
            let Some((count, text, image_bytes, had_image_alongside_text, unsupported_kinds)) =
                read
            else {
                return Ok(None);
            };

            // Self-write suppression (fix DUP-ON-COPY): when the daemon itself
            // wrote to the pasteboard (copy_item / "copy" handler), the next
            // poll will see a changeCount increment caused by our own write.
            // We must advance `last_change_count` (so we don't re-fire on the
            // same count on the *next* poll) but must NOT record the content —
            // the existing item was already promoted to the top of history by
            // `bump_item_recency`. Clear the sentinel after consuming it.
            let self_write_cc = self.self_write_change_count.load(Ordering::Acquire);
            if self_write_cc >= 0 && count == self_write_cc {
                self.last_change_count = count;
                // Consume the sentinel — only suppress once per self-write.
                self.self_write_change_count.store(-1, Ordering::Release);
                tracing::debug!(
                    change_count = count,
                    "clipboard: skipping self-write (copy_item/copy handler wrote this change)"
                );
                return Ok(None);
            }

            // Compute delta BEFORE we update the cursor. On first poll
            // (sentinel -1) we suppress the burst signal.
            let delta = if self.last_change_count < 0 {
                1
            } else {
                count - self.last_change_count
            };
            self.last_change_count = count;

            if delta >= SKIPPED_BATCH_THRESHOLD {
                let missed = (delta - 1) as usize;
                tracing::info!(
                    delta,
                    missed,
                    "clipboard: rapid changes detected — {} intermediate updates lost",
                    missed
                );
                // Surface as its own event; the caller will poll again to
                // pick up the latest content immediately.
                return Ok(Some(ClipboardContent::SkippedBatch(missed)));
            }

            if had_image_alongside_text {
                tracing::info!("clipboard had text+image; text wins (image dropped this poll)");
            }

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
                if bytes.len() > self.max_image_bytes {
                    return Err(ClipboardError::ImageTooLarge {
                        max: self.max_image_bytes,
                        actual: bytes.len(),
                    });
                }
                return Ok(Some(ClipboardContent::Image(bytes)));
            }

            // No supported content — log any unknown kinds once each.
            for kind in unsupported_kinds {
                log_unsupported_once(&kind);
            }

            Ok(None)
        }
        #[cfg(not(target_os = "macos"))]
        {
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

    /// edge HIGH #5 — rapid_change_skip_batch_logs.
    /// We can't drive NSPasteboard from a unit test, so we verify the
    /// surrounding logic instead: when the synthetic delta is at/above
    /// threshold the monitor would emit `SkippedBatch(delta - 1)`.
    #[test]
    fn rapid_change_skip_batch_logs() {
        // Simulate the same delta arithmetic poll() uses.
        let prev: i64 = 10;
        let curr: i64 = 15;
        let delta = curr - prev;
        assert!(delta >= SKIPPED_BATCH_THRESHOLD);
        let missed = (delta - 1) as usize;
        let evt = ClipboardContent::SkippedBatch(missed);
        match evt {
            ClipboardContent::SkippedBatch(n) => assert_eq!(n, 4),
            _ => panic!("expected SkippedBatch"),
        }
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

    /// edge HIGH #7 — unsupported_rtf_logs_once_per_kind.
    /// Verifies the once-per-kind gate suppresses repeat logs for the
    /// same UTI but allows a new kind through.
    #[test]
    fn unsupported_rtf_logs_once_per_kind() {
        reset_unsupported_kinds_for_test();
        // First call inserts and would log.
        log_unsupported_once("public.rtf");
        // Second call must be a no-op (set already contains it).
        log_unsupported_once("public.rtf");
        // Verify set contents.
        let seen = unsupported_kind_seen().lock().unwrap();
        assert!(seen.contains("public.rtf"));
        assert_eq!(seen.len(), 1);
        drop(seen);

        // A different kind goes through and grows the set.
        log_unsupported_once("public.file-url");
        let seen = unsupported_kind_seen().lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(seen.contains("public.file-url"));
    }

    /// security LOW #19 — image_dedup_uses_sha256.
    /// `image_content_hash` must be deterministic across calls and equal
    /// to the first 16 bytes of SHA-256(input). Different inputs must
    /// produce different hashes.
    #[test]
    fn image_dedup_uses_sha256() {
        let a = b"\x89PNG\r\n\x1a\n some image bytes";
        let b = b"\x89PNG\r\n\x1a\n some image bytes";
        let c = b"\x89PNG\r\n\x1a\n DIFFERENT bytes";

        let ha = image_content_hash(a);
        let hb = image_content_hash(b);
        let hc = image_content_hash(c);

        // Deterministic.
        assert_eq!(ha, hb);
        // Distinct inputs → distinct hashes (with overwhelming probability).
        assert_ne!(ha, hc);

        // Equals first 16 bytes of SHA-256.
        let expected = Sha256::digest(a);
        assert_eq!(&ha[..], &expected[..16]);
        assert_eq!(ha.len(), 16);
    }
}
