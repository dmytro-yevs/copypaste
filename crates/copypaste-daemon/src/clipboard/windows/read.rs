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

use crate::clipboard::CapturePolicy;

/// Allowance on the pre-read text gate, in bytes. See [`text`].
const SIZE_SLACK: u64 = 4096;

/// One representation, read but not yet converted.
pub(super) enum Representation {
    Text(String),
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

/// v2 captures plain text only. Other formats are acknowledged by the parent
/// change cursor without being materialised.
pub(super) fn representation(policy: CapturePolicy<'_>) -> Reading {
    if raw::is_format_avail(formats::CF_UNICODETEXT) {
        return text(policy);
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
