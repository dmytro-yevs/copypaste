//! Choosing and reading one clipboard representation.
//!
//! Separate from the poll protocol because the two change for different
//! reasons: this owns the format vocabulary and the pre-read size gates, the
//! parent owns the change cursor and the gates that decide whether to read at
//! all. It holds no state, so an oversized item is reported here and counted
//! there (I-39) rather than growing a second counter.
//!
//! Every function here requires the caller to hold the clipboard open.

use clipboard_win::{formats, raw};
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
pub(super) fn representation(policy: CapturePolicy<'_>, registered: &RegisteredFormats) -> Reading {
    if raw::is_format_avail(formats::CF_HDROP) {
        return file();
    }
    if raw::is_format_avail(formats::CF_UNICODETEXT) {
        return text(policy);
    }
    if let Some(format) = registered
        .rtf
        .filter(|format| raw::is_format_avail(*format))
    {
        return formatted_text(policy, format, copypaste_ipc::content_type::RICH_TEXT);
    }
    if let Some(format) = registered
        .html
        .filter(|format| raw::is_format_avail(*format))
    {
        return html(policy, format);
    }
    if let Some(format) = registered
        .png
        .filter(|format| raw::is_format_avail(*format))
    {
        return image(policy, format, ImageKind::Png);
    }
    if let Some(format) = registered
        .tiff
        .filter(|format| raw::is_format_avail(*format))
    {
        return image(policy, format, ImageKind::Tiff);
    }
    if raw::is_format_avail(formats::CF_DIB) {
        return dib(policy);
    }
    Reading::Nothing
}

fn text(policy: CapturePolicy<'_>) -> Reading {
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

fn formatted_text(policy: CapturePolicy<'_>, format: u32, content_type: &'static str) -> Reading {
    let cap = policy.limit_bytes(content_type);
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    if bytes > cap {
        return Reading::TooLarge { bytes, cap };
    }
    let mut bytes = Vec::new();
    if raw::get_vec(format, &mut bytes).is_err() {
        debug!("the formatted clipboard text could not be read; the change was dropped");
        return Reading::Nothing;
    }
    if bytes.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: bytes.len() as u64,
            cap,
        };
    }
    normalized_text(bytes, content_type, cap)
}

fn html(policy: CapturePolicy<'_>, format: u32) -> Reading {
    let cap = policy.limit_bytes(copypaste_ipc::content_type::HTML);
    let Some(bytes) = preflight(format) else {
        return Reading::Nothing;
    };
    if bytes > cap {
        return Reading::TooLarge { bytes, cap };
    }
    let mut payload = Vec::new();
    if raw::get_vec(format, &mut payload).is_err() {
        debug!("the clipboard HTML could not be read; the change was dropped");
        return Reading::Nothing;
    }
    if payload.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: payload.len() as u64,
            cap,
        };
    }
    // GlobalSize can include non-UTF-8 allocation padding beyond EndFragment.
    let Some(fragment) = crate::clipboard::cf_html::fragment(&payload) else {
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

fn image(policy: CapturePolicy<'_>, format: u32, kind: ImageKind) -> Reading {
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

fn dib(policy: CapturePolicy<'_>) -> Reading {
    let cap = policy.limit_bytes(copypaste_ipc::content_type::IMAGE_PNG);
    let Some(bytes) = preflight(formats::CF_DIB) else {
        return Reading::Nothing;
    };
    if bytes > cap {
        return Reading::TooLarge { bytes, cap };
    }
    let mut dib = Vec::new();
    if raw::get_vec(formats::CF_DIB, &mut dib).is_err() {
        debug!("the clipboard bitmap could not be read; the change was dropped");
        return Reading::Nothing;
    }
    if dib.len() as u64 > cap {
        return Reading::TooLarge {
            bytes: dib.len() as u64,
            cap,
        };
    }
    let reading = image_read(
        transcode::png_from_dib(&dib, policy.settings.max_decoded_image_mb, cap),
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

fn file() -> Reading {
    let mut paths = Vec::new();
    if raw::get_file_list_path(&mut paths).is_err() {
        debug!("the clipboard file list could not be read; the change was dropped");
        return Reading::Nothing;
    }
    let [path] = paths.as_slice() else {
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
