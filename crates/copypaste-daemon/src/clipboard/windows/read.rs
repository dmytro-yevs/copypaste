//! Choosing and reading one clipboard representation.
//!
//! Separate from the poll protocol because the two change for different
//! reasons: this owns the format vocabulary and the pre-read size gates, the
//! parent owns the change cursor and the gates that decide whether to read at
//! all. It holds no state, so an oversized item is reported here and counted
//! there (I-39) rather than growing a second counter.
//!
//! The selected reader owns the clipboard guard and closes it as soon as its
//! native bytes have been copied. Decoding and normalization must not prevent
//! other applications from copying or pasting.

use clipboard_win::{formats, raw, Clipboard};
use tracing::debug;
use typed_path::{Utf8WindowsPath, Utf8WindowsPrefix};

use crate::clipboard::CapturePolicy;

use super::transcode::{self, CapturedImage};

/// Allowance on the pre-read text gate, in bytes. See [`text`].
const SIZE_SLACK: u64 = 4096;

/// One representation, read but not yet converted.
pub(super) enum Representation {
    Text {
        content: String,
        content_type: &'static str,
    },
    Image {
        bytes: Vec<u8>,
        content_type: &'static str,
    },
    File {
        path: std::path::PathBuf,
        metadata: copypaste_core::FileMetadata,
    },
}

/// Clipboard atoms registered once when the backend starts. They are stable for
/// the process lifetime and probing them does not materialise clipboard data.
pub(super) struct RegisteredFormats {
    excluded: Option<u32>,
    concealed: Option<u32>,
    transient: Option<u32>,
    history: Option<u32>,
    png: Option<u32>,
    tiff: Option<u32>,
    rtf: Option<u32>,
    html: Option<u32>,
}

impl RegisteredFormats {
    pub(super) fn privacy(&self) -> copypaste_ipc::ClipboardPrivacy {
        if [self.excluded, self.concealed, self.transient, self.history]
            .iter()
            .any(Option::is_none)
        {
            return copypaste_ipc::ClipboardPrivacy {
                secret: true,
                transient: true,
            };
        }
        let present = |format: Option<u32>| format.is_some_and(raw::is_format_avail);
        let excluded_from_history = self
            .history
            .filter(|format| raw::is_format_avail(*format))
            .is_some_and(|format| {
                let mut value = [0u8; 4];
                raw::size(format).map(|size| size.get()) != Some(value.len())
                    || raw::get(format, &mut value).ok() != Some(value.len())
                    || u32::from_le_bytes(value) == 0
            });
        copypaste_ipc::ClipboardPrivacy {
            secret: present(self.excluded) || present(self.concealed),
            transient: present(self.transient) || excluded_from_history,
        }
    }
    pub(super) fn register() -> Self {
        Self {
            excluded: raw::register_format("ExcludeClipboardContentFromMonitorProcessing")
                .map(|value| value.get()),
            concealed: raw::register_format("org.nspasteboard.ConcealedType")
                .map(|value| value.get()),
            transient: raw::register_format("org.nspasteboard.TransientType")
                .map(|value| value.get()),
            history: raw::register_format("CanIncludeInClipboardHistory").map(|value| value.get()),
            png: raw::register_format("PNG").map(|format| format.get()),
            tiff: raw::register_format("TIFF").map(|format| format.get()),
            rtf: raw::register_format("Rich Text Format").map(|format| format.get()),
            html: raw::register_format("HTML Format").map(|format| format.get()),
        }
    }
}

pub(super) enum Reading {
    Got(Representation),
    /// Nothing was read. The caller counts it (I-39 / §6.5) and drops the
    /// change.
    TooLarge {
        bytes: u64,
        cap: u64,
    },
    DecodedTooLarge {
        megabytes: u32,
    },
    Nothing,
}

/// A native file reference wins over its textual fallback. The remaining
/// precedence is Unicode text, RTF, HTML, PNG, TIFF, then a native bitmap.
/// Availability is checked before each read, so an
/// earlier representation never materialises a later one.
pub(super) fn representation(
    policy: CapturePolicy<'_>,
    registered: &RegisteredFormats,
    clipboard: Clipboard,
) -> Reading {
    if raw::is_format_avail(formats::CF_HDROP) {
        return file(policy, clipboard);
    }
    if raw::is_format_avail(formats::CF_UNICODETEXT) {
        return text(policy, clipboard);
    }
    if let Some(format) = registered
        .rtf
        .filter(|format| raw::is_format_avail(*format))
    {
        return formatted_text(
            policy,
            format,
            copypaste_ipc::content_type::RICH_TEXT,
            clipboard,
        );
    }
    if let Some(format) = registered
        .html
        .filter(|format| raw::is_format_avail(*format))
    {
        return html(policy, format, clipboard);
    }
    if let Some(format) = registered
        .png
        .filter(|format| raw::is_format_avail(*format))
    {
        return image(policy, format, ImageKind::Png, clipboard);
    }
    if let Some(format) = registered
        .tiff
        .filter(|format| raw::is_format_avail(*format))
    {
        return image(policy, format, ImageKind::Tiff, clipboard);
    }
    for format in [formats::CF_DIBV5, formats::CF_DIB] {
        if raw::is_format_avail(format) {
            return dib(policy, format, clipboard);
        }
    }
    Reading::Nothing
}

fn text(policy: CapturePolicy<'_>, clipboard: Clipboard) -> Reading {
    let cap = policy.limit_bytes(copypaste_ipc::content_type::TEXT);
    let Some(utf16_bytes) = raw::size(formats::CF_UNICODETEXT) else {
        return Reading::Nothing;
    };
    let utf16_bytes = utf16_bytes.get() as u64;
    // I-18 in UTF-16 arithmetic. The clipboard holds UTF-16 and the cap counts
    // UTF-8 bytes, so one code unit is at least one UTF-8 byte and twice the cap
    // is what cannot possibly fit. The slack is the terminating NUL and whatever
    // `GlobalSize` rounds the allocation up to: a real Windows heap rounded an
    // exact-boundary allocation by more than 16 bytes. This gate exists to
    // bound the copy, not to enforce the cap — that happens exactly, below, on
    // the converted string — so it errs towards reading.
    if utf16_bytes > cap.saturating_mul(2).saturating_add(SIZE_SLACK) {
        return Reading::TooLarge {
            bytes: utf16_bytes,
            cap,
        };
    }

    let mut bytes = Vec::new();
    if raw::get_string(&mut bytes).is_err() {
        debug!("the clipboard text could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    if bytes.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: bytes.len() as u64,
            cap,
        };
    }
    // `WideCharToMultiByte` already substitutes for unpaired surrogates; §3.6's
    // precedent is lossy conversion rather than dropping the user's copy, and
    // I-37 forbids panicking on a malformed payload.
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if text.is_empty() {
        return Reading::Nothing;
    }
    Reading::Got(Representation::Text {
        content: text,
        content_type: copypaste_ipc::content_type::TEXT,
    })
}

fn formatted_text(
    policy: CapturePolicy<'_>,
    format: u32,
    content_type: &'static str,
    clipboard: Clipboard,
) -> Reading {
    let cap = policy.limit_bytes(content_type);
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    // Registered text can include a NUL terminator and GlobalSize allocation
    // padding. Enforce the retained-content cap after normalization, as on
    // macOS, while bounding this native copy independently.
    if bytes > text_allocation_limit(cap) {
        return Reading::TooLarge { bytes, cap };
    }
    let mut bytes = Vec::new();
    if raw::get_vec(format, &mut bytes).is_err() {
        debug!("the formatted clipboard text could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    if bytes.len() as u64 > text_allocation_limit(cap) {
        return Reading::TooLarge {
            bytes: bytes.len() as u64,
            cap,
        };
    }
    normalized_text(bytes, content_type, cap)
}

fn html(policy: CapturePolicy<'_>, format: u32, clipboard: Clipboard) -> Reading {
    let cap = policy.limit_bytes(copypaste_ipc::content_type::HTML);
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    // CF_HTML adds a byte-offset header around the stored HTML fragment.
    // That transport envelope is not part of the retained-content budget.
    if bytes > text_allocation_limit(cap) {
        return Reading::TooLarge { bytes, cap };
    }
    let mut payload = Vec::new();
    if raw::get_vec(format, &mut payload).is_err() {
        debug!("the clipboard HTML could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    html_read(&payload, cap)
}

fn text_allocation_limit(cap: u64) -> u64 {
    cap.saturating_add(SIZE_SLACK)
}

fn html_read(payload: &[u8], cap: u64) -> Reading {
    if payload.len() as u64 > text_allocation_limit(cap) {
        return Reading::TooLarge {
            bytes: payload.len() as u64,
            cap,
        };
    }
    // GlobalSize can include non-UTF-8 allocation padding beyond EndFragment.
    let Some(fragment) = crate::clipboard::cf_html::fragment(payload) else {
        debug!("the clipboard HTML fragment is invalid; the change was dropped");
        return Reading::Nothing;
    };
    if fragment.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: fragment.len() as u64,
            cap,
        };
    }
    normalized_text(fragment.to_vec(), copypaste_ipc::content_type::HTML, cap)
}

#[cfg(test)]
mod text_tests {
    use super::*;

    fn html_payload(fragment: &str) -> Vec<u8> {
        let header = "Version:1.0\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
        let header = header
            .replace(
                "StartFragment:0000000000",
                &format!("StartFragment:{:010}", header.len()),
            )
            .replace(
                "EndFragment:0000000000",
                &format!("EndFragment:{:010}", header.len() + fragment.len()),
            );
        [header.as_bytes(), fragment.as_bytes(), &[0, 0xff, 0xfe]].concat()
    }

    #[test]
    fn html_at_the_retained_limit_survives_transport_header_and_padding() {
        let fragment = "<b>Привіт</b>";
        let payload = html_payload(fragment);
        let Reading::Got(Representation::Text {
            content,
            content_type,
        }) = html_read(&payload, fragment.len() as u64)
        else {
            panic!("transport bytes must not consume the stored HTML budget");
        };
        assert_eq!(content, fragment);
        assert_eq!(content_type, copypaste_ipc::content_type::HTML);
        assert!(matches!(
            html_read(&payload, fragment.len() as u64 - 1),
            Reading::TooLarge { .. }
        ));
    }

    #[test]
    fn html_native_allocation_is_still_bounded() {
        let mut payload = html_payload("x");
        payload.resize(text_allocation_limit(1) as usize + 1, 0);
        assert!(matches!(html_read(&payload, 1), Reading::TooLarge { .. }));
        assert!(matches!(html_read(b"invalid", 100), Reading::Nothing));
    }

    #[test]
    fn formatted_text_at_the_limit_ignores_native_nul_termination() {
        let rtf = b"{\\rtf1 hello}";
        let mut payload = rtf.to_vec();
        payload.extend_from_slice(&[0; 16]);
        let Reading::Got(Representation::Text {
            content,
            content_type,
        }) = normalized_text(
            payload.clone(),
            copypaste_ipc::content_type::RICH_TEXT,
            rtf.len() as u64,
        )
        else {
            panic!("native termination must not consume the stored RTF budget");
        };
        assert_eq!(content.as_bytes(), rtf);
        assert_eq!(content_type, copypaste_ipc::content_type::RICH_TEXT);
        assert!(matches!(
            normalized_text(payload, content_type, rtf.len() as u64 - 1),
            Reading::TooLarge { .. }
        ));
    }
}

fn normalized_text(bytes: Vec<u8>, content_type: &'static str, cap: u64) -> Reading {
    let content = String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .to_owned();
    if content.is_empty() {
        return Reading::Nothing;
    }
    if content.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: content.len() as u64,
            cap,
        };
    }
    Reading::Got(Representation::Text {
        content,
        content_type,
    })
}

#[derive(Clone, Copy)]
enum ImageKind {
    Png,
    Tiff,
}

fn image(policy: CapturePolicy<'_>, format: u32, kind: ImageKind, clipboard: Clipboard) -> Reading {
    let cap = policy.limit_bytes(copypaste_ipc::content_type::IMAGE_PNG);
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    if bytes > cap {
        return Reading::TooLarge { bytes, cap };
    }
    let mut encoded = Vec::new();
    if raw::get_vec(format, &mut encoded).is_err() {
        debug!("the clipboard image could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    if encoded.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: encoded.len() as u64,
            cap,
        };
    }
    image_read(
        transcode::checked_image(
            encoded,
            match kind {
                ImageKind::Png => image::ImageFormat::Png,
                ImageKind::Tiff => image::ImageFormat::Tiff,
            },
            policy.settings.max_decoded_image_mb,
        ),
        cap,
        match kind {
            ImageKind::Png => copypaste_ipc::content_type::IMAGE_PNG,
            ImageKind::Tiff => copypaste_ipc::content_type::IMAGE_TIFF,
        },
        policy.settings.max_decoded_image_mb,
    )
}

fn dib_allocation_limit(policy: CapturePolicy<'_>) -> u64 {
    // DIB is uncompressed. The PNG retention cap must apply after encoding,
    // otherwise even a 1080p screenshot is rejected before it can compress.
    u64::from(policy.settings.max_decoded_image_mb)
        .saturating_mul(1024 * 1024)
        .saturating_add(64 * 1024)
}

fn dib(policy: CapturePolicy<'_>, format: u32, clipboard: Clipboard) -> Reading {
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    if bytes > dib_allocation_limit(policy) {
        return Reading::DecodedTooLarge {
            megabytes: policy.settings.max_decoded_image_mb,
        };
    }
    let mut dib = Vec::new();
    if raw::get_vec(format, &mut dib).is_err() {
        debug!("the clipboard bitmap could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    dib_read(policy, &dib)
}

fn dib_read(policy: CapturePolicy<'_>, dib: &[u8]) -> Reading {
    if dib.len() as u64 > dib_allocation_limit(policy) {
        return Reading::DecodedTooLarge {
            megabytes: policy.settings.max_decoded_image_mb,
        };
    }
    let cap = policy.limit_bytes(copypaste_ipc::content_type::IMAGE_PNG);
    let reading = image_read(
        transcode::png_from_dib(dib, policy.settings.max_decoded_image_mb, cap),
        cap,
        copypaste_ipc::content_type::IMAGE_PNG,
        policy.settings.max_decoded_image_mb,
    );
    match reading {
        Reading::Got(Representation::Image { ref bytes, .. }) if bytes.len() as u64 > cap => {
            Reading::TooLarge {
                bytes: bytes.len() as u64,
                cap,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod dib_tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};

    fn screenshot() -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(
            1920,
            1080,
            Rgb([0x24u8, 0x65, 0xa8]),
        ));
        let mut bitmap = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bitmap, ImageFormat::Bmp).unwrap();
        bitmap.into_inner()[14..].to_vec()
    }

    #[test]
    fn dibv5_screenshot_reads_pixels_immediately_after_its_header() {
        // A real CF_DIBV5 layout: 124-byte BITMAPV5HEADER containing the
        // BI_BITFIELDS masks, followed immediately by one BGRA pixel.
        let mut dib = vec![0u8; 124];
        dib[0..4].copy_from_slice(&124u32.to_le_bytes());
        dib[4..8].copy_from_slice(&1i32.to_le_bytes());
        dib[8..12].copy_from_slice(&1i32.to_le_bytes());
        dib[12..14].copy_from_slice(&1u16.to_le_bytes());
        dib[14..16].copy_from_slice(&32u16.to_le_bytes());
        dib[16..20].copy_from_slice(&3u32.to_le_bytes());
        dib[20..24].copy_from_slice(&4u32.to_le_bytes());
        for (offset, mask) in [
            (40, 0x00ff0000u32),
            (44, 0x0000ff00),
            (48, 0x000000ff),
            (52, 0xff000000),
        ] {
            dib[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
        }
        dib.extend_from_slice(&[51, 34, 17, 63]);
        let settings = copypaste_ipc::ConfigData::default();
        let Reading::Got(Representation::Image { bytes, .. }) =
            dib_read(CapturePolicy::new(&settings), &dib)
        else {
            panic!("a Windows DIBV5 screenshot must survive native capture");
        };
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .into_rgba8()
                .get_pixel(0, 0)
                .0,
            [17, 34, 51, 63]
        );
    }

    #[test]
    fn full_hd_screenshot_larger_than_png_cap_is_captured() {
        let settings = copypaste_ipc::ConfigData::default();
        let policy = CapturePolicy::new(&settings);
        let dib = screenshot();
        assert!(dib.len() as u64 > policy.limit_bytes(copypaste_ipc::content_type::IMAGE_PNG));
        assert!(dib.len() as u64 <= dib_allocation_limit(policy));
        let Reading::Got(Representation::Image {
            bytes,
            content_type,
        }) = dib_read(policy, &dib)
        else {
            panic!("a normal screenshot must survive the complete DIB read boundary");
        };
        assert_eq!(content_type, copypaste_ipc::content_type::IMAGE_PNG);
        let image = image::load_from_memory(&bytes).unwrap();
        assert_eq!((image.width(), image.height()), (1920, 1080));
        assert!(bytes.len() as u64 <= settings.max_image_size_bytes);
    }

    #[test]
    fn screenshot_still_obeys_decoded_memory_budget() {
        let settings = copypaste_ipc::ConfigData {
            max_decoded_image_mb: 1,
            ..Default::default()
        };
        assert!(matches!(
            dib_read(CapturePolicy::new(&settings), &screenshot()),
            Reading::DecodedTooLarge { megabytes: 1 }
        ));
    }

    #[test]
    fn screenshot_still_obeys_encoded_budget() {
        let settings = copypaste_ipc::ConfigData {
            max_image_size_bytes: 64,
            ..Default::default()
        };
        assert!(matches!(
            dib_read(CapturePolicy::new(&settings), &screenshot()),
            Reading::TooLarge { cap: 64, .. }
        ));
    }
}

fn file(policy: CapturePolicy<'_>, clipboard: Clipboard) -> Reading {
    let Some(bytes) = preflight(formats::CF_HDROP) else {
        return Reading::Nothing;
    };
    let cap = file_allocation_limit(policy);
    if bytes > cap {
        return Reading::TooLarge { bytes, cap };
    }
    let mut paths = Vec::new();
    if raw::get_file_list_path(&mut paths).is_err() {
        debug!("the clipboard file list could not be read; the change was dropped");
        return Reading::Nothing;
    }
    drop(clipboard);
    file_reference(&paths)
}

fn file_allocation_limit(policy: CapturePolicy<'_>) -> u64 {
    // This is a list of references, not file bodies. A multi-GiB file-body
    // allowance must never become a multi-GiB native metadata allocation.
    policy
        .limit_bytes(copypaste_ipc::content_type::FILE)
        .min(copypaste_ipc::MAX_CONTENT_BYTES as u64)
}

fn file_reference(paths: &[std::path::PathBuf]) -> Reading {
    let [path] = paths else {
        debug!("the clipboard file list is not a single file; the change was dropped");
        return Reading::Nothing;
    };
    let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
        debug!("the clipboard file name could not be used; the change was dropped");
        return Reading::Nothing;
    };
    let Some(metadata) = copypaste_core::FileMetadata::with_source_reference(
        filename,
        "application/octet-stream",
        path.to_string_lossy(),
    ) else {
        debug!("the clipboard file metadata is invalid; the change was dropped");
        return Reading::Nothing;
    };
    if !is_local_disk_path(path) {
        debug!("the clipboard file reference is not a local disk path; the change was dropped");
        return Reading::Nothing;
    }
    Reading::Got(Representation::File {
        path: path.clone(),
        metadata,
    })
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn large_file_body_policy_does_not_expand_native_reference_allocations() {
        let settings = copypaste_ipc::ConfigData {
            max_file_size_bytes: copypaste_ipc::MAX_FILE_SIZE_BYTES,
            ..Default::default()
        };
        assert_eq!(
            file_allocation_limit(CapturePolicy::new(&settings)),
            copypaste_ipc::MAX_CONTENT_BYTES as u64
        );
    }

    #[test]
    fn one_local_file_retains_its_unicode_reference_without_reading_its_body() {
        let path = PathBuf::from(r"C:\synthetic\Україна.txt");
        let Reading::Got(Representation::File {
            path: reference,
            metadata,
        }) = file_reference(&[path.clone()])
        else {
            panic!("a local file reference must be accepted without touching the file");
        };
        assert_eq!(reference, path);
        assert_eq!(metadata.filename, "Україна.txt");
        assert_eq!(metadata.source_reference.as_deref(), path.to_str());
    }

    #[test]
    fn unsupported_file_lists_and_nonlocal_references_are_rejected() {
        for paths in [
            vec![],
            vec![PathBuf::from(r"C:\one.txt"), PathBuf::from(r"C:\two.txt")],
            vec![PathBuf::from(r"\\server\share\one.txt")],
            vec![PathBuf::from(r"\\.\pipe\synthetic")],
            vec![PathBuf::from(r"relative\one.txt")],
        ] {
            assert!(matches!(file_reference(&paths), Reading::Nothing));
        }
    }
}

fn is_local_disk_path(path: &std::path::Path) -> bool {
    let Some(raw_path) = path.to_str() else {
        return false;
    };
    let windows_path = Utf8WindowsPath::new(raw_path);
    let prefix = windows_path
        .components()
        .next()
        .and_then(|component| component.prefix_kind());
    path.is_absolute()
        && matches!(
            prefix,
            Some(Utf8WindowsPrefix::Disk(_) | Utf8WindowsPrefix::VerbatimDisk(_))
        )
}

fn preflight(format: u32) -> Option<u64> {
    raw::size(format).map(|size| size.get() as u64).or_else(|| {
        debug!("the clipboard representation size could not be read; the change was dropped");
        None
    })
}

fn image_read(
    image: CapturedImage,
    cap: u64,
    content_type: &'static str,
    decoded_memory_mb: u32,
) -> Reading {
    match image {
        CapturedImage::Image(bytes) => Reading::Got(Representation::Image {
            bytes,
            content_type,
        }),
        CapturedImage::TooLarge(bytes) => Reading::TooLarge { bytes, cap },
        // The decoder limit is a byte budget, but an image format does not
        // expose its exact decoded size safely before decode. The caller
        // counts this rejection without logging an invented byte count.
        CapturedImage::DecodedTooLarge => Reading::DecodedTooLarge {
            megabytes: decoded_memory_mb,
        },
        CapturedImage::Invalid => Reading::Nothing,
    }
}
