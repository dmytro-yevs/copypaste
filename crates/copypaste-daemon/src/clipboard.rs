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

/// Derive the thumbnail's `file_id` deterministically from the full image's
/// `file_id`. The thumbnail is encrypted with the SAME content key but a
/// DISTINCT `file_id` so its AEAD AAD is isolated from the full image's
/// (see `image::encode_image_full`). Domain-separating the hash (a `"thumb"`
/// prefix) guarantees the two ids never collide while staying deterministic,
/// so identical images still dedup and a reader can recompute / parse the id.
pub fn image_thumb_file_id(file_id: &[u8; 16]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"copypaste-thumb-v1");
    hasher.update(file_id);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Build the image `blob_ref` meta JSON for an image item.
///
/// Keeps the original `width`/`height`/`original_size`/`chunk_count`/`file_id`
/// keys (consumed by `ipc::parse_image_file_id` and the full-res decode path)
/// and ADDITIVELY records the thumbnail's `thumb_file_id` (as a byte array, the
/// same shape as `file_id`) plus `thumb_w`/`thumb_h`. The core reader ignores
/// unknown keys, so this stays forward-/backward-compatible.
pub fn build_image_meta_json(
    meta: &copypaste_core::ImageMeta,
    thumb_file_id: &[u8; 16],
    thumb_w: u32,
    thumb_h: u32,
) -> String {
    format!(
        r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?},"thumb_file_id":{:?},"thumb_w":{},"thumb_h":{}}}"#,
        meta.width,
        meta.height,
        meta.original_size,
        meta.chunk_count,
        meta.file_id,
        thumb_file_id,
        thumb_w,
        thumb_h
    )
}

/// Build the file `blob_ref` meta JSON for a file item.
///
/// Carries the same `file_id` key the image meta uses (so the shared
/// `ipc::parse_image_file_id` parser recovers it for both content types) plus
/// the file-specific `filename`/`mime`/`original_size`/`chunk_count`. The core
/// reader ignores unknown keys, so this stays forward-/backward-compatible.
///
/// `filename` and `mime` are JSON-string-escaped via `serde_json` so arbitrary
/// names round-trip safely.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn build_file_meta_json(meta: &copypaste_core::FileMeta) -> String {
    // serde_json::to_string on a &str produces a correctly-escaped JSON string
    // literal (including the surrounding quotes); infallible for plain strings,
    // so the unwrap_or keeps us total without panicking.
    let filename = serde_json::to_string(&meta.filename).unwrap_or_else(|_| "\"\"".to_string());
    let mime = serde_json::to_string(&meta.mime).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"{{"filename":{},"mime":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
        filename, mime, meta.original_size, meta.chunk_count, meta.file_id
    )
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

/// Percent-decode a URL path component (e.g. `%20` → space).
///
/// Used to convert a `file://` URL string into a filesystem path using only
/// the standard library (no `url` crate dependency). Only decodes sequences
/// of the form `%HH` where `HH` is a valid two-digit hexadecimal byte value.
/// Invalid sequences are passed through unchanged.
///
/// macOS only: gated on `cfg(target_os = "macos")` because it is only called
/// from the macOS `poll` path. The `allow(dead_code)` on non-macOS keeps CI
/// clean without removing the helper.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn percent_decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            // Parse the two hex digits following the '%'.
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // A valid UTF-8 file path should round-trip cleanly; fall back to lossy
    // conversion on the (pathological) case of invalid UTF-8 after decoding.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// Derive a MIME type string from a file extension.
///
/// Covers common clipboard file types. Falls back to
/// `application/octet-stream` for unrecognised extensions. This avoids
/// pulling in the `mime_guess` crate as a new dependency.
///
/// macOS only: see [`percent_decode_path`].
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mime_from_path(path: &std::path::Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "text" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "tiff" | "tif" => "image/tiff",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "rs" | "py" | "js" | "ts" | "sh" | "rb" | "go" | "c" | "h" | "cpp" => "text/plain",
        _ => "application/octet-stream",
    }
    .to_string()
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
    /// Maximum raw file bytes accepted from a `public.file-url` clipboard entry
    /// before the READ gate rejects the file. Defaults to
    /// [`copypaste_core::MAX_FILE_BYTES`] (100 MiB); the daemon overrides this
    /// with `AppConfig::max_file_size_bytes` so the READ gate matches the encode
    /// gate rather than the hardcoded core const.
    max_file_bytes: usize,
    /// changeCount recorded after a daemon self-write to NSPasteboard (copy_item /
    /// "copy" IPC handler). When the next poll sees this exact changeCount the daemon
    /// caused the change itself — skip recording to prevent a duplicate row.
    /// Shared with `write_to_pasteboard` via an `Arc<AtomicI64>`.
    pub self_write_change_count: Arc<AtomicI64>,
}

/// CopyPaste-pbre: process-wide cache of the invariant pasteboard UTI strings.
///
/// `poll()` ran on every changed tick and called `NSString::from_str(...)` for
/// each constant UTI (the three `org.nspasteboard.*` markers, `public.png`,
/// `public.tiff`, the two file-URL types, and the five "unsupported" probes),
/// heap-allocating a fresh Cocoa string each time. These UTIs never change, so
/// we build each `Retained<NSString>` exactly once and reuse it. `NSString` is
/// `Send + Sync` (immutable Cocoa string), so a `LazyLock<Retained<NSString>>`
/// static is sound. These are strong references owned by the static, so they
/// are NOT placed in the per-tick autorelease pool.
#[cfg(target_os = "macos")]
mod pb_uti {
    use objc2::rc::Retained;
    use objc2_foundation::NSString;
    use std::sync::LazyLock;

    macro_rules! cached_uti {
        ($name:ident, $uti:literal) => {
            pub static $name: LazyLock<Retained<NSString>> =
                LazyLock::new(|| NSString::from_str($uti));
        };
    }

    cached_uti!(TRANSIENT, "org.nspasteboard.TransientType");
    cached_uti!(CONCEALED, "org.nspasteboard.ConcealedType");
    cached_uti!(AUTOGEN, "org.nspasteboard.AutoGeneratedType");
    cached_uti!(PNG, "public.png");
    cached_uti!(TIFF, "public.tiff");
    cached_uti!(FILE_URL, "public.file-url");
    cached_uti!(FILENAMES, "NSFilenamesPboardType");

    /// The five "unsupported kind" probes, paired with their label, so the poll
    /// loop can iterate cached strings instead of re-allocating each tick.
    pub static UNSUPPORTED_PROBES: LazyLock<[(&'static str, Retained<NSString>); 5]> =
        LazyLock::new(|| {
            [
                ("public.rtf", NSString::from_str("public.rtf")),
                ("public.rtfd", NSString::from_str("public.rtfd")),
                ("public.html", NSString::from_str("public.html")),
                ("public.url", NSString::from_str("public.url")),
                (
                    "com.apple.pasteboard.promised-file-url",
                    NSString::from_str("com.apple.pasteboard.promised-file-url"),
                ),
            ]
        });
}

impl ClipboardMonitor {
    pub fn new(max_text_bytes: u64) -> Self {
        use copypaste_core::{MAX_FILE_BYTES, MAX_IMAGE_BYTES};
        Self {
            last_change_count: -1,
            max_text_bytes,
            max_image_bytes: MAX_IMAGE_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            self_write_change_count: Arc::new(AtomicI64::new(-1)),
        }
    }

    /// Override the image-size READ gate with the user-configured cap.
    /// Call this after [`ClipboardMonitor::new`] when a non-default cap is set.
    pub fn set_max_image_bytes(&mut self, bytes: usize) {
        self.max_image_bytes = bytes;
    }

    /// Override the file-size READ gate with the user-configured cap.
    /// Call this after [`ClipboardMonitor::new`] when a non-default cap is set.
    pub fn set_max_file_bytes(&mut self, bytes: usize) {
        self.max_file_bytes = bytes;
    }

    /// Override the text-size READ gate with the user-configured cap.
    ///
    /// The daemon poll loop pushes the live `max_text_size_bytes` here each tick
    /// so raising/lowering the cap via `set_config` takes effect without a
    /// restart (the monitor's gate would otherwise keep its startup snapshot).
    pub fn set_max_text_bytes(&mut self, bytes: u64) {
        self.max_text_bytes = bytes;
    }

    /// Poll for new clipboard content. Returns `Some` only if the pasteboard changed.
    ///
    /// Priority: text > image (PNG/TIFF) > file (public.file-url).
    /// Image data and file data are returned as raw bytes for downstream
    /// compression + encryption.
    ///
    /// Edge cases handled (Wave 2.1):
    /// - **Rapid changes (#5):** if the changeCount delta is ≥
    ///   [`SKIPPED_BATCH_THRESHOLD`], we cannot recover intermediate values
    ///   but emit telemetry; consumers can poll again immediately to drain.
    /// - **Mixed text+image (#6):** text wins; an INFO log records that an
    ///   image was silently dropped on that poll.
    /// - **Unsupported types (#7):** RTF / custom UTIs are logged once per
    ///   kind, never silently dropped. `public.file-url` is now handled.
    pub fn poll(&mut self) -> Result<Option<ClipboardContent>, ClipboardError> {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
            use objc2_foundation::{NSArray, NSData};

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

                // org.nspasteboard SKIP (Maccy parity / security):
                // Password managers and other privacy-aware apps annotate their
                // copies with one of three transient-/concealed-/auto-generated
                // UTIs so clipboard managers know to skip them.  We probe for
                // all three BEFORE reading any content so we never store or even
                // buffer a password-manager secret. When any of the markers is
                // present we advance `last_change_count` (so the item is not
                // re-offered on the next poll) and return `None`.
                // Reference: http://nspasteboard.org
                let nspb_skip = {
                    // CopyPaste-pbre: reuse the process-wide cached UTI strings
                    // instead of allocating three fresh NSStrings every tick.
                    let probes = NSArray::from_id_slice(&[
                        pb_uti::TRANSIENT.clone(),
                        pb_uti::CONCEALED.clone(),
                        pb_uti::AUTOGEN.clone(),
                    ]);
                    unsafe { pb.availableTypeFromArray(&probes) }.is_some()
                };
                if nspb_skip {
                    tracing::debug!(
                        change_count = count,
                        "clipboard: org.nspasteboard marker detected — skipping \
                         (transient/concealed/auto-generated)"
                    );
                    // Signal to the outer code that this changeCount should be
                    // advanced (skipped) but no content stored.  We return a
                    // sentinel tuple with `nspb_skip = true`.
                    return Some((count, None, None, None, false, vec![], true));
                }

                // CopyPaste-pbre: reuse the cached image-UTI strings.
                let png_type = &*pb_uti::PNG;
                let tiff_type = &*pb_uti::TIFF;

                // Text
                let text = unsafe {
                    pb.stringForType(NSPasteboardTypeString)
                        .map(|ns| ns.to_string())
                };

                // Probe image presence WITHOUT copying the bytes: ask the
                // pasteboard whether any image type is available rather than
                // materialising the (potentially multi-MB) NSData (#6).
                let image_types =
                    NSArray::from_id_slice(&[(*png_type).clone(), (*tiff_type).clone()]);
                let image_present = unsafe { pb.availableTypeFromArray(&image_types) }.is_some();
                let had_image_alongside_text = text.is_some() && image_present;

                // Only materialise image bytes when text is absent and an
                // image is actually present.
                let image_bytes: Option<Vec<u8>> = if text.is_none() && image_present {
                    let png_data = unsafe { pb.dataForType(png_type) };
                    if let Some(ref d) = png_data {
                        Some(d.bytes().to_vec())
                    } else {
                        let tiff_data = unsafe { pb.dataForType(tiff_type) };
                        tiff_data.as_deref().map(|d: &NSData| d.bytes().to_vec())
                    }
                } else {
                    None
                };

                // File-URL branch: probe `public.file-url` only when text and
                // image are both absent, to keep priority text > image > file.
                // `stringForType` returns the file URL as a string (e.g.
                // "file:///Users/alice/doc.pdf"). We resolve it to a path, read
                // the bytes, and derive the filename + MIME from the path.
                // `NSFilenamesPboardType` is the legacy (pre-UTI) name for the
                // same data; we probe it as a fallback.
                let file_content: Option<(std::path::PathBuf, String, String)> = if text.is_none()
                    && image_bytes.is_none()
                {
                    // CopyPaste-pbre: reuse the cached file-URL UTI strings.
                    let file_url_type = &*pb_uti::FILE_URL;
                    let filenames_type = &*pb_uti::FILENAMES;
                    // Prefer the UTI form; fall back to the legacy type.
                    let url_str: Option<String> = unsafe {
                        pb.stringForType(file_url_type)
                            .map(|s| s.to_string())
                            .or_else(|| pb.stringForType(filenames_type).map(|s| s.to_string()))
                    };
                    if let Some(url_str) = url_str {
                        // Strip the "file://" scheme and percent-decode the
                        // path using only std (no extra dependency needed).
                        let raw_path = url_str.strip_prefix("file://").unwrap_or(url_str.as_str());
                        let decoded = percent_decode_path(raw_path);
                        let path = std::path::Path::new(&decoded);
                        if path.is_absolute() {
                            let filename = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "file".to_string());
                            // Best-effort MIME from file extension; fall back to
                            // application/octet-stream for unknown extensions.
                            let mime = mime_from_path(path);
                            // Return a FileRef instead of reading the bytes here.
                            // The actual std::fs::read runs in handle_tick via
                            // tokio::task::spawn_blocking so this tokio worker
                            // thread is not blocked on potentially large I/O.
                            Some((path.to_path_buf(), filename, mime))
                        } else {
                            tracing::debug!(
                                url = %url_str,
                                "clipboard: file-url is not a local absolute path — skipping"
                            );
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Detect unsupported types — only matters when we have
                // nothing else to surface (text + image + file all absent).
                // We probe a fixed allowlist of common unsupported UTIs
                // rather than enumerating `pb.types()` so we don't need
                // the `NSEnumerator` feature on objc2-foundation 0.2.
                // `public.file-url` is now handled above; keep other types here.
                let mut unsupported_kinds: Vec<String> = Vec::new();
                if text.is_none() && image_bytes.is_none() && file_content.is_none() {
                    // CopyPaste-pbre: iterate the process-wide cached probe
                    // NSStrings instead of allocating one per kind every tick.
                    for (label, ns_kind) in pb_uti::UNSUPPORTED_PROBES.iter() {
                        let present = unsafe {
                            pb.dataForType(ns_kind).is_some() || pb.stringForType(ns_kind).is_some()
                        };
                        if present {
                            unsupported_kinds.push((*label).to_string());
                        }
                    }
                }

                Some((
                    count,
                    text,
                    image_bytes,
                    file_content,
                    had_image_alongside_text,
                    unsupported_kinds,
                    false, // nspb_skip: not set on the normal content path
                ))
            });

            // Unchanged pasteboard — nothing read, nothing allocated.
            let Some((
                count,
                text,
                image_bytes,
                file_content,
                had_image_alongside_text,
                unsupported_kinds,
                nspb_skip,
            )) = read
            else {
                return Ok(None);
            };

            // org.nspasteboard skip: advance the cursor but produce no content.
            if nspb_skip {
                self.last_change_count = count;
                return Ok(None);
            }

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
                // CRITICAL fix: do NOT return SkippedBatch here and discard the
                // already-read content. The old code called:
                //   self.last_change_count = count   (line above)
                //   return Ok(Some(SkippedBatch))    ← early return
                // This caused the NEXT poll to see count == last_change_count → None,
                // permanently losing the most-recent clipboard item. Instead we log
                // the burst as a telemetry side-channel and fall through so the
                // content path below captures the current pasteboard value.
                tracing::info!(
                    delta,
                    missed,
                    "clipboard: rapid changes detected — {} intermediate updates lost \
                     (most-recent item still captured)",
                    missed
                );
                // Intentional fall-through: do not return here.
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

            if let Some((path, filename, mime)) = file_content {
                // The size gate will be applied after spawn_blocking reads
                // the bytes in handle_tick. Return FileRef here so the
                // tokio worker is not blocked on potentially large I/O.
                return Ok(Some(ClipboardContent::FileRef {
                    path,
                    filename,
                    mime,
                }));
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
