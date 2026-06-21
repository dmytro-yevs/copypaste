//! Raw image codec helpers: decode (clipboard → `DynamicImage`), encode (→ PNG),
//! allocation-limit construction, and the RGBA8 thumbnail pixel helper.

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

use super::ImageError;

/// Build `image::Limits` from a decoded-bytes budget (MB).
///
/// `max_alloc` is set to `max_decoded_mb * 1024 * 1024` bytes.  That single
/// cap is sufficient to prevent decode-bomb OOM: the `image` crate enforces
/// `max_alloc` before the pixel buffer is allocated regardless of image
/// dimensions.
///
/// Per-axis `max_image_width` / `max_image_height` are intentionally *not*
/// set here.  Deriving them from a square-image assumption (√(budget/4))
/// incorrectly rejects valid wide/tall images (e.g. 4096×32) whose total
/// pixel count is far below the memory budget.  The allocation cap is the
/// authoritative guard; the per-axis fields are redundant and harmful.
pub(super) fn make_limits(max_decoded_mb: u32) -> Limits {
    // Saturating arithmetic: an absurdly large config value should just give a
    // very generous limit rather than wrap or panic.
    let max_bytes = (max_decoded_mb as u64).saturating_mul(1024 * 1024);
    // Limits is #[non_exhaustive] so we cannot use a struct literal — start
    // from the default and overwrite only the allocation cap.
    let mut limits = Limits::default();
    limits.max_alloc = Some(max_bytes);
    limits
}

/// Decode raw clipboard bytes (PNG or TIFF) into a `DynamicImage`.
///
/// Enforces `image::Limits` capped at [`crate::config::MAX_DECODED_IMAGE_MB`] (50 MB) so that
/// a highly-compressed "decode-bomb" image is rejected before any large
/// allocation occurs.  To supply a custom cap from `AppConfig` call
/// [`decode_clipboard_image_limited`] instead.
///
/// Tries PNG first, then TIFF.  Returns `ImageError::UnsupportedFormat` if
/// neither decode succeeds.
pub fn decode_clipboard_image(raw: &[u8]) -> Result<DynamicImage, ImageError> {
    decode_clipboard_image_limited(raw, crate::config::MAX_DECODED_IMAGE_MB)
}

/// Like [`decode_clipboard_image`] but accepts an explicit allocation cap in
/// megabytes.  Pass `AppConfig::max_decoded_image_mb` here when you have a
/// config available; otherwise prefer `decode_clipboard_image` which uses the
/// compile-time default.
pub fn decode_clipboard_image_limited(
    raw: &[u8],
    max_decoded_mb: u32,
) -> Result<DynamicImage, ImageError> {
    if raw.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    let limits = make_limits(max_decoded_mb);

    // Try PNG
    let mut png_reader = ImageReader::with_format(Cursor::new(raw), ImageFormat::Png);
    png_reader.limits(limits.clone());
    if let Ok(img) = png_reader.decode() {
        return Ok(img);
    }

    // Try TIFF (macOS often puts TIFF on pasteboard)
    let mut tiff_reader = ImageReader::with_format(Cursor::new(raw), ImageFormat::Tiff);
    tiff_reader.limits(limits.clone());
    if let Ok(img) = tiff_reader.decode() {
        return Ok(img);
    }

    // Generic sniff (handles BMP, etc.)
    let mut generic_reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|e| ImageError::Decode(e.to_string()))?;
    generic_reader.limits(limits);
    generic_reader
        .decode()
        .map_err(|e| ImageError::Decode(e.to_string()))
}

/// Decode raw image bytes (PNG/TIFF/BMP) and produce an RGBA8 thumbnail
/// that fits within `(max_w, max_h)`, preserving aspect ratio.
///
/// Enforces the same [`crate::config::MAX_DECODED_IMAGE_MB`] allocation cap as
/// [`decode_clipboard_image`] so decode-bomb inputs are rejected before any
/// large allocation.
///
/// Returns `(rgba_bytes, width, height)` where `rgba_bytes.len() == width * height * 4`.
///
/// If the source image already fits the bounds the original pixels are
/// returned (still as RGBA8), so callers always get a uniform pixel format
/// suitable for display as an RGBA8 image.
///
/// This is an additive Wave 3.4 helper used by the HistoryWindow to render
/// inline previews of clipboard images without leaking the full bitmap
/// through IPC.
pub fn thumbnail(
    raw_bytes: &[u8],
    max_w: u32,
    max_h: u32,
) -> Result<(Vec<u8>, u32, u32), ImageError> {
    if max_w == 0 || max_h == 0 {
        return Err(ImageError::Decode(
            "thumbnail bounds must be non-zero".into(),
        ));
    }

    let img = decode_clipboard_image(raw_bytes)?;
    let (w, h) = img.dimensions();
    let resized = if w > max_w || h > max_h {
        img.thumbnail(max_w, max_h)
    } else {
        img
    };
    let rgba = resized.to_rgba8();
    let (rw, rh) = (rgba.width(), rgba.height());
    Ok((rgba.into_raw(), rw, rh))
}

/// Re-encode a `DynamicImage` as PNG bytes (lossless, pure-Rust).
pub fn encode_as_png(img: &DynamicImage) -> Result<Vec<u8>, ImageError> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| ImageError::Encode(e.to_string()))?;
    Ok(buf.into_inner())
}
