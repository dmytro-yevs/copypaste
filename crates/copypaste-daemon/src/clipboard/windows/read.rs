//! Choosing and reading one clipboard representation.
//!
//! Separate from the poll protocol because the two change for different
//! reasons: this owns the format vocabulary and the pre-read size gates, the
//! parent owns the change cursor and the gates that decide whether to read at
//! all. It holds no state, so an oversized item is reported here and counted
//! there (I-39) rather than growing a second counter.
//!
//! Every function here requires the caller to hold the clipboard open.

use std::path::PathBuf;

use clipboard_win::{formats, raw};
use tracing::debug;

use crate::clipboard::CapturePolicy;

/// Allowance on the pre-read text gate, in bytes. See [`text`].
const SIZE_SLACK: u64 = 4096;

/// One representation, read but not yet converted.
pub(super) enum Representation {
    Text(String),
    /// A whole BMP file — file header, info header, bits — which is what
    /// `raw::get_bitmap` renders the clipboard's bitmap into.
    Bitmap(Vec<u8>),
    File(PathBuf),
}

pub(super) enum Reading {
    Got(Representation),
    /// Nothing was read. The caller counts it (I-39 / §6.5) and drops the
    /// change.
    TooLarge {
        bytes: u64,
        cap: u64,
    },
    Nothing,
}

/// I-11: one representation, chosen in text, image, file order.
pub(super) fn representation(policy: CapturePolicy<'_>) -> Reading {
    if raw::is_format_avail(formats::CF_UNICODETEXT) {
        return text(policy);
    }
    if raw::is_format_avail(formats::CF_DIB) {
        return image(policy);
    }
    if raw::is_format_avail(formats::CF_HDROP) {
        return file();
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
    Reading::Got(Representation::Text(text))
}

fn image(policy: CapturePolicy<'_>) -> Reading {
    // Sized through `CF_DIB` and read through `CF_BITMAP`: Windows synthesises
    // each from the other, so availability of one implies both, and `GlobalSize`
    // on a GDI bitmap handle is the case clipboard-win's own `size_unsafe`
    // warns can crash.
    let Some(dib_bytes) = raw::size(formats::CF_DIB) else {
        return Reading::Nothing;
    };
    // Gated against the *decoded* budget rather than the stored-image cap: a
    // DIB is uncompressed, so a screenshot that stores as a 400 KiB PNG arrives
    // here as several MiB, and gating it on the PNG cap would reject ordinary
    // screenshots. The encoded PNG is checked against that cap once it exists.
    let budget = u64::from(policy.settings.max_decoded_image_mb).saturating_mul(1024 * 1024);
    let dib_bytes = dib_bytes.get() as u64;
    if dib_bytes > budget {
        return Reading::TooLarge {
            bytes: dib_bytes,
            cap: budget,
        };
    }

    let mut bitmap = Vec::new();
    if raw::get_bitmap(&mut bitmap).is_err() {
        debug!("the clipboard image could not be read; the change was dropped");
        return Reading::Nothing;
    }
    Reading::Got(Representation::Bitmap(bitmap))
}

fn file() -> Reading {
    let mut paths: Vec<PathBuf> = Vec::new();
    if raw::get_file_list_path(&mut paths).is_err() {
        debug!("the clipboard file list could not be read; the change was dropped");
        return Reading::Nothing;
    }
    // One item per change, as on macOS: the first absolute path.
    paths
        .into_iter()
        .find(|path| path.is_absolute())
        .map_or(Reading::Nothing, |path| {
            Reading::Got(Representation::File(path))
        })
}
