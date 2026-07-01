//! macOS pasteboard helpers: once-per-kind "unsupported type" logging, and
//! the `file://` URL → filesystem path / MIME helpers used by `poll()`'s
//! `public.file-url` and `NSFilenamesPboardType` handling.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Process-wide set of pasteboard kinds we've already logged once.
/// Keeps the steady-state log volume bounded when the user repeatedly
/// copies an unsupported type (e.g. RTF inside a text editor).
pub(super) fn unsupported_kind_seen() -> &'static Mutex<HashSet<String>> {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Log an unsupported clipboard kind at INFO level, but only the first
/// time we see each distinct kind in this process.
pub(super) fn log_unsupported_once(kind: &str) {
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
pub(super) fn reset_unsupported_kinds_for_test() {
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
pub(super) fn percent_decode_path(s: &str) -> String {
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
pub(super) fn mime_from_path(path: &std::path::Path) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// CopyPaste-q5ab: NSFilenamesPboardType binary plist parsing.
    ///
    /// Verifies that a binary plist serialised from a Vec<String> of absolute
    /// POSIX paths is correctly round-tripped by `plist::from_bytes::<Vec<String>>`,
    /// which is the codec used in the NSFilenamesPboardType fallback path.
    ///
    /// We construct the plist payload using `plist::to_writer_binary` (the same
    /// encoding Apple uses on macOS) and then parse it back, mimicking exactly
    /// what the clipboard path does.
    #[cfg(target_os = "macos")]
    #[test]
    fn nsfilenames_pboard_type_plist_roundtrip() {
        // Simulate the binary plist payload that Finder/apps put on
        // NSFilenamesPboardType: an NSArray of absolute POSIX path strings.
        let paths: Vec<String> = vec![
            "/Users/alice/Documents/report.pdf".to_string(),
            "/Users/alice/Downloads/photo.png".to_string(),
        ];

        // Encode to binary plist (the format macOS uses on the pasteboard).
        let mut buf: Vec<u8> = Vec::new();
        plist::to_writer_binary(&mut buf, &paths).expect("plist encode must not fail");

        // Now parse it the same way the production clipboard path does.
        let recovered: Vec<String> = plist::from_bytes(&buf).expect("plist decode must not fail");

        assert_eq!(recovered.len(), 2, "should recover both paths");
        assert_eq!(recovered[0], "/Users/alice/Documents/report.pdf");
        assert_eq!(recovered[1], "/Users/alice/Downloads/photo.png");

        // Confirm that the first path is treated as absolute (the production
        // gate).
        let first = std::path::PathBuf::from(&recovered[0]);
        assert!(
            first.is_absolute(),
            "first recovered path must be absolute: {first:?}"
        );
    }

    /// CopyPaste-q5ab: a non-plist payload (e.g. garbage bytes that sometimes
    /// appear on the pasteboard from third-party apps) must not panic — the
    /// production path logs a debug message and returns None.
    #[cfg(target_os = "macos")]
    #[test]
    fn nsfilenames_pboard_type_malformed_plist_is_silent() {
        let garbage = b"this is not a plist";
        let result = plist::from_bytes::<Vec<String>>(garbage);
        assert!(
            result.is_err(),
            "malformed payload must return Err so the production path skips it"
        );
    }
}
