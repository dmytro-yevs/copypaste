//! Image compression, chunking, and encryption pipeline for clipboard images.
//!
//! Pipeline (encode):
//!   raw PNG/TIFF bytes  →  decode with `image` crate
//!                       →  re-encode as PNG (lossless, portable)
//!                       →  split into 512 KB chunks
//!                       →  encrypt each chunk with XChaCha20-Poly1305
//!
//! Pipeline (decode):
//!   encrypted chunks  →  decrypt  →  reassemble  →  PNG bytes
//!
//! The module is intentionally platform-agnostic: NSPasteboard reading lives
//! in `copypaste-daemon`, so all code here is testable without macOS.

use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits};
use std::io::Cursor;
use thiserror::Error;

use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};

/// 512 KB chunk size.
pub const IMAGE_CHUNK_SIZE: usize = 512 * 1024;
/// Maximum accepted image size (raw bytes before compression): 10 MB.
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

/// Longest-side bound (px) for the capture-time encrypted thumbnail.
///
/// The thumbnail is generated once at capture from the SAME decoded
/// `DynamicImage` as the full-resolution blob (see [`encode_image_full`]) and
/// stored encrypted in the `thumb` column (schema v9). The UI renders this in a
/// ~40 px list row, so a 192 px longest side stays crisp at 2× retina while
/// keeping the *decoded* bitmap area small: a WebView decodes the data URI to
/// an RGBA bitmap whose RSS scales with dim² (192² is ≈ 12.5× less decoded
/// memory than the previous 680² source). This is the dominant fix for HB-10
/// (350 MB image-memory) — the UI LRU caps compressed data-URI string bytes,
/// not the decoded bitmaps, so the source dimension is what actually bounds
/// RSS. See [`thumb_dims_exceed_cap`] for the backfill regeneration gate.
pub const THUMBNAIL_MAX_DIM: u32 = 192;

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("Image too large: {actual} bytes (max {max})")]
    TooLarge { actual: usize, max: usize },
    #[error("Unsupported image format")]
    UnsupportedFormat,
    #[error("Image decode error: {0}")]
    Decode(String),
    #[error("Image encode error: {0}")]
    Encode(String),
    #[error("Chunk encryption error: {0}")]
    Chunk(#[from] ChunkError),
}

/// Metadata stored alongside encrypted chunks.
#[derive(Debug, Clone)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
    /// Original raw byte count (before compression).
    pub original_size: u64,
    /// Number of encrypted chunks.
    pub chunk_count: u32,
    /// UUID-derived file_id used as AAD context for chunk encryption.
    pub file_id: [u8; 16],
}

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
fn make_limits(max_decoded_mb: u32) -> Limits {
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
/// Enforces `image::Limits` capped at [`MAX_DECODED_IMAGE_MB`] (50 MB) so that
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
/// Enforces the same [`MAX_DECODED_IMAGE_MB`] allocation cap as
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

/// Full encode pipeline:
///   raw clipboard bytes → decode → PNG → split into chunks → encrypt
///
/// `max_bytes` is the configured raw-byte ceiling (the daemon threads
/// `AppConfig::max_image_size_bytes` here). Passing `0` falls back to the
/// library default [`MAX_IMAGE_BYTES`] so callers without config still get a
/// sane bound.
///
/// Enforces the [`make_limits`] allocation cap (derived from
/// [`crate::config::MAX_DECODED_IMAGE_MB`]) to prevent decode-bomb OOM before
/// the pixel buffer is allocated.
///
/// Also checks the re-encoded PNG size against `max_bytes` to prevent
/// amplification: a highly-compressed input can pass the raw-byte gate,
/// decode successfully (bounded by `image::Limits`), and then re-encode to a
/// lossless PNG that is 3–4× larger than the original compressed bytes.
/// Without this second gate the oversized PNG blob flows to
/// `encrypt_chunks`/SQLite unchecked.
///
/// Returns `(ImageMeta, Vec<EncryptedChunk>)`.
///
/// For a version that also accepts a custom decoded-bytes budget (MB) see
/// [`encode_image_with_limit`].
pub fn encode_image(
    raw: &[u8],
    key: &[u8; 32],
    file_id: &[u8; 16],
    max_bytes: usize,
) -> Result<(ImageMeta, Vec<EncryptedChunk>), ImageError> {
    // Convenience wrapper: uses the compile-time MAX_DECODED_IMAGE_MB default. The
    // real capture path threads the user-configured budget via
    // `encode_image_with_limit`; this entry point is for callers that have no
    // config to pass (tests, internal re-encodes).
    encode_image_with_limit(
        raw,
        key,
        file_id,
        max_bytes,
        crate::config::MAX_DECODED_IMAGE_MB,
    )
}

/// Like [`encode_image`] but accepts an explicit `max_decoded_mb` allocation
/// cap for the decode step, allowing the daemon (or tests) to thread the
/// user-configured `AppConfig::max_decoded_image_mb` value all the way
/// through to `decode_clipboard_image_limited`.
///
/// `max_bytes` gates both the raw-input size and the re-encoded PNG size (to
/// prevent decode-amplification; see [`encode_image`] doc).
pub fn encode_image_with_limit(
    raw: &[u8],
    key: &[u8; 32],
    file_id: &[u8; 16],
    max_bytes: usize,
    max_decoded_mb: u32,
) -> Result<(ImageMeta, Vec<EncryptedChunk>), ImageError> {
    let max = if max_bytes == 0 {
        MAX_IMAGE_BYTES
    } else {
        max_bytes
    };
    if raw.len() > max {
        return Err(ImageError::TooLarge {
            actual: raw.len(),
            max,
        });
    }

    let original_size = raw.len() as u64;

    // [PERF] Scope the decoded `DynamicImage` to this block so it (≈ width *
    // height * 4 bytes — tens of MB for a 4K image) is freed the instant the
    // PNG buffer is produced, well before chunk encryption allocates its own
    // Vecs. Holding the bitmap + PNG bytes + chunk Vecs simultaneously was the
    // encode-path memory peak. Only `(width, height)` and the PNG bytes outlive
    // the block.
    let (width, height, png_bytes) = {
        // Use the caller-supplied decoded-bytes budget so AppConfig::max_decoded_image_mb
        // is honoured rather than the hardcoded compile-time constant.
        let img = decode_clipboard_image_limited(raw, max_decoded_mb)?;
        let (width, height) = (img.width(), img.height());
        let png_bytes = encode_as_png(&img)?;
        (width, height, png_bytes)
        // `img` drops here, before encrypt_chunks runs below.
    };

    // [HIGH] Guard against decode-amplification: a highly-compressed input
    // passes the raw-byte gate above, decodes within the Limits budget, and
    // then re-encodes to a lossless PNG that may be 3–4× larger. Without this
    // second check the oversized PNG blob would flow to encrypt_chunks/SQLite.
    if png_bytes.len() > max {
        return Err(ImageError::TooLarge {
            actual: png_bytes.len(),
            max,
        });
    }

    let chunks = encrypt_chunks(&png_bytes, key, file_id, IMAGE_CHUNK_SIZE)?;
    // [PERF] The PNG bytes now live in the chunk ciphertexts; free the
    // intermediate buffer before building meta / returning so it does not
    // co-reside with the chunk Vecs any longer than necessary.
    drop(png_bytes);
    // [LOW] chunks.len() is provably ≤ ceil(max / IMAGE_CHUNK_SIZE) ≤
    // ceil(MAX_IMAGE_BYTES / 512 KiB) = 20, so it always fits in a u32.
    // Use try_from + expect to make the invariant explicit and catch any
    // future refactor that widens the gate.
    let chunk_count =
        u32::try_from(chunks.len()).map_err(|_| ImageError::Chunk(ChunkError::TooManyChunks))?;

    let meta = ImageMeta {
        width,
        height,
        original_size,
        chunk_count,
        file_id: *file_id,
    };

    Ok((meta, chunks))
}

/// Full decode pipeline:
///   encrypted chunks → decrypt → reassemble → PNG bytes
///
/// The caller is responsible for writing PNG bytes back to NSPasteboard.
pub fn decode_image(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<Vec<u8>, ImageError> {
    let png_bytes = decrypt_chunks(chunks, key, file_id)?;
    Ok(png_bytes)
}

/// Encode a small preview thumbnail of `img` into an encrypted chunk blob.
///
/// Pipeline:
///   `img.thumbnail(max_dim, max_dim)` (downscale only — never upscales, and
///   preserves aspect ratio)
///     → encode to bytes
///     → [`encrypt_chunks`] keyed by `thumb_file_id` (a DISTINCT file_id from
///       the full image's, so the thumbnail's AEAD AAD is isolated)
///     → [`chunks_to_blob`]
///
/// Returns the serialized blob ready for the `clipboard_items.thumb` BLOB
/// column. Use [`decode_thumbnail`] to recover the encoded bytes.
///
/// ## Thumbnail image format
///
/// The thumbnail is encoded as **PNG**, not WebP. The production `image` crate
/// dependency is pinned to `features = ["png", "tiff"]` (see root `Cargo.toml`)
/// and does not enable a WebP *encoder* (the `image` 0.25 WebP support is
/// decode-only / lossless and the encoder was dropped upstream). Per the Phase
/// 1 plan we fall back to PNG rather than widening the dependency's feature set
/// in a core primitive. PNG keeps the thumbnail lossless and pure-Rust; a
/// future phase may revisit a lossy codec if thumbnail byte size becomes a
/// concern.
pub fn encode_thumbnail(
    img: &DynamicImage,
    key: &[u8; 32],
    thumb_file_id: &[u8; 16],
    max_dim: u32,
) -> Result<Vec<u8>, ImageError> {
    let (bytes, _w, _h) = encode_thumbnail_bytes(img, max_dim)?;
    let chunks = encrypt_chunks(&bytes, key, thumb_file_id, IMAGE_CHUNK_SIZE)?;
    chunks_to_blob(&chunks)
}

/// Downscale `img` to fit `(max_dim, max_dim)` (never upscaling) and encode the
/// result as PNG bytes. Returns `(png_bytes, width, height)` of the thumbnail.
///
/// Shared by [`encode_thumbnail`] and [`encode_image_full`] so the thumbnail
/// dimensions can be recorded in the image meta without re-deriving them.
fn encode_thumbnail_bytes(
    img: &DynamicImage,
    max_dim: u32,
) -> Result<(Vec<u8>, u32, u32), ImageError> {
    // `DynamicImage::thumbnail` scales to FIT the bound and will UPSCALE a
    // source smaller than the bound. We only ever want to shrink, so guard:
    // when the source already fits within `(max_dim, max_dim)` keep it as-is.
    let (sw, sh) = img.dimensions();
    let thumb = if sw > max_dim || sh > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img.clone()
    };
    let (w, h) = thumb.dimensions();
    let bytes = encode_as_png(&thumb)?;
    Ok((bytes, w, h))
}

/// Decode an encrypted thumbnail blob (produced by [`encode_thumbnail`]) back
/// into the encoded image bytes (PNG). Mirrors [`decode_image`].
///
/// `thumb_file_id` MUST be the same value passed to [`encode_thumbnail`]; the
/// chunk AEAD binds it as AAD, so a wrong id fails the integrity check rather
/// than returning garbage.
pub fn decode_thumbnail(
    blob: &[u8],
    key: &[u8; 32],
    thumb_file_id: &[u8; 16],
) -> Result<Vec<u8>, ImageError> {
    let chunks = chunks_from_blob(blob)?;
    let bytes = decrypt_chunks(&chunks, key, thumb_file_id)?;
    Ok(bytes)
}

/// Lazy-backfill path: decode raw PNG bytes, then produce an encrypted
/// thumbnail blob identical in format to the capture-time path.
///
/// This is the Phase-4 helper used by `get_item_thumbnail` when a legacy
/// image item has `thumb IS NULL`. The caller decrypts the full-res content
/// to `png_bytes` first (via [`decode_image`]), then passes them here.
///
/// Pipeline:
///   `png_bytes` → [`decode_clipboard_image`] → [`encode_thumbnail`]
///
/// On success returns `(thumb_blob, thumb_w, thumb_h)` — the same shape
/// produced by [`encode_image_full`] — so the caller can persist it via
/// [`crate::storage::items::set_thumb`] and record `thumb_w`/`thumb_h` in
/// the updated `blob_ref` meta JSON.
///
/// `thumb_file_id` MUST be the DISTINCT id derived from the full-image
/// `file_id` (see `clipboard::image_thumb_file_id`); it is bound as AEAD
/// AAD so the same id MUST be used when decoding (via [`decode_thumbnail`]).
pub fn encode_thumbnail_from_png(
    png_bytes: &[u8],
    key: &[u8; 32],
    thumb_file_id: &[u8; 16],
    max_dim: u32,
) -> Result<(Vec<u8>, u32, u32), ImageError> {
    let img = decode_clipboard_image(png_bytes)?;
    let (bytes, w, h) = encode_thumbnail_bytes(&img, max_dim)?;
    let chunks = encrypt_chunks(&bytes, key, thumb_file_id, IMAGE_CHUNK_SIZE)?;
    let blob = chunks_to_blob(&chunks)?;
    Ok((blob, w, h))
}

/// True when a *stored* thumbnail whose recorded dimensions are
/// `(thumb_w, thumb_h)` is larger than the current [`THUMBNAIL_MAX_DIM`] cap on
/// its longest side and should therefore be regenerated.
///
/// Used by the lazy-backfill path: a thumbnail persisted under an older, larger
/// cap (e.g. 680 px) must be re-shrunk to the current cap so the UI never
/// decodes an oversized bitmap (HB-10). Returns `false` for an already-conformant
/// thumbnail so conformant rows are never needlessly re-encoded.
///
/// Zero dimensions (missing/corrupt meta) return `false`: there is nothing
/// trustworthy to compare against, and the caller's normal NULL-thumb backfill
/// path already handles genuinely absent thumbnails.
pub fn thumb_dims_exceed_cap(thumb_w: u32, thumb_h: u32) -> bool {
    if thumb_w == 0 || thumb_h == 0 {
        return false;
    }
    thumb_w.max(thumb_h) > THUMBNAIL_MAX_DIM
}

/// Full capture-time encode producing BOTH the full-resolution encrypted
/// chunks AND a small encrypted thumbnail blob from a SINGLE decode of `raw`.
///
/// This is the Variant-B entry point: decoding clipboard bytes is the
/// expensive step, so we decode once and reuse the same [`DynamicImage`] for
/// the full PNG re-encode and the downscaled thumbnail — no second decode.
///
/// Returns `(meta, full_chunks, thumb_blob, thumb_w, thumb_h)` where:
///   * `meta` / `full_chunks` are exactly what [`encode_image_with_limit`]
///     produces (same gates, same `file_id` AAD).
///   * `thumb_blob` is the serialized encrypted thumbnail keyed by the
///     SEPARATE `thumb_file_id` (distinct AAD from the full image).
///   * `thumb_w` / `thumb_h` are the thumbnail's pixel dimensions.
///
/// `file_id` and `thumb_file_id` MUST differ so the full image and its
/// thumbnail have independent AEAD contexts. The caller is responsible for
/// generating two distinct ids.
///
/// [`encode_image_with_limit`] is left intact for back-compat callers that do
/// not need a thumbnail.
#[allow(clippy::type_complexity)] // 5-tuple is the documented Phase-1 return contract; a named struct lands in Phase 2 with the IPC wiring.
pub fn encode_image_full(
    raw: &[u8],
    key: &[u8; 32],
    file_id: &[u8; 16],
    thumb_file_id: &[u8; 16],
    max_bytes: usize,
    max_decoded_mb: u32,
    thumb_max_dim: u32,
) -> Result<(ImageMeta, Vec<EncryptedChunk>, Vec<u8>, u32, u32), ImageError> {
    let max = if max_bytes == 0 {
        MAX_IMAGE_BYTES
    } else {
        max_bytes
    };
    if raw.len() > max {
        return Err(ImageError::TooLarge {
            actual: raw.len(),
            max,
        });
    }

    // Decode ONCE; reuse for both the full re-encode and the thumbnail.
    let img = decode_clipboard_image_limited(raw, max_decoded_mb)?;
    let (width, height) = (img.width(), img.height());
    let original_size = raw.len() as u64;

    let png_bytes = encode_as_png(&img)?;
    // Same decode-amplification guard as encode_image_with_limit.
    if png_bytes.len() > max {
        return Err(ImageError::TooLarge {
            actual: png_bytes.len(),
            max,
        });
    }

    let chunks = encrypt_chunks(&png_bytes, key, file_id, IMAGE_CHUNK_SIZE)?;
    let chunk_count =
        u32::try_from(chunks.len()).map_err(|_| ImageError::Chunk(ChunkError::TooManyChunks))?;

    // Thumbnail reuses the already-decoded image — no second decode.
    let (thumb_bytes, thumb_w, thumb_h) = encode_thumbnail_bytes(&img, thumb_max_dim)?;
    let thumb_chunks = encrypt_chunks(&thumb_bytes, key, thumb_file_id, IMAGE_CHUNK_SIZE)?;
    let thumb_blob = chunks_to_blob(&thumb_chunks)?;

    let meta = ImageMeta {
        width,
        height,
        original_size,
        chunk_count,
        file_id: *file_id,
    };

    Ok((meta, chunks, thumb_blob, thumb_w, thumb_h))
}

/// Serialize chunks to a flat byte blob for SQLite BLOB storage.
///
/// Format: `[chunk_count: u32 BE] [chunk_0_wire] [chunk_1_wire] ...`
///
/// Returns `Err(ImageError::Chunk(ChunkError::TooManyChunks))` if the slice
/// is somehow longer than `u32::MAX` (cannot happen via `encrypt_chunks` which
/// enforces the same bound, but avoids a panic on a direct call with an
/// oversized slice).
pub fn chunks_to_blob(chunks: &[EncryptedChunk]) -> Result<Vec<u8>, ImageError> {
    let count =
        u32::try_from(chunks.len()).map_err(|_| ImageError::Chunk(ChunkError::TooManyChunks))?;
    // F3: pre-size the output so the repeated `extend_from_slice` below never
    // reallocs (which, for a multi-MiB blob, spikes peak memory to ~2x). The
    // exact layout is:
    //   4 (count) + Σ over chunks of [ 4 (wire-len prefix) + wire_len ]
    // where wire_len = 34 header bytes ([version:1][index:4][is_final:1]
    // [nonce:24][ct_len:4]) + ciphertext.len(). Computed from `ciphertext.len()`
    // directly so we do NOT allocate a throwaway `to_wire()` just to measure it.
    // `usize` math cannot overflow in practice: `count` fits in u32 and each
    // ciphertext is bounded by the chunk size, so the sum is far below usize::MAX.
    const WIRE_HEADER_LEN: usize = 1 + 4 + 1 + 24 + 4; // = 34
    let total: usize = 4 + chunks
        .iter()
        .map(|c| 4 + WIRE_HEADER_LEN + c.ciphertext.len())
        .sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&count.to_be_bytes());
    for chunk in chunks {
        let wire = chunk.to_wire();
        out.extend_from_slice(&(wire.len() as u32).to_be_bytes());
        out.extend_from_slice(&wire);
    }
    debug_assert_eq!(out.len(), total, "chunks_to_blob presize must be exact");
    Ok(out)
}

/// Deserialize chunks from the SQLite BLOB format produced by `chunks_to_blob`.
pub fn chunks_from_blob(blob: &[u8]) -> Result<Vec<EncryptedChunk>, ImageError> {
    use crate::crypto::chunks::CHUNK_FORMAT_VERSION;

    if blob.len() < 4 {
        return Err(ImageError::Decode("blob too short".into()));
    }
    let count = u32::from_be_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;

    // Smallest possible per-chunk footprint in the blob: a 4-byte wire-length
    // prefix plus the minimum wire header [version:1][index:4][is_final:1]
    // [nonce:24][ct_len:4] = 34 bytes, i.e. 38 bytes total. A declared `count`
    // can therefore never exceed `(blob.len() - 4) / 38`. We clamp the reserve
    // against this ceiling so a corrupt/malicious blob with a huge count field
    // cannot trigger a multi-GB `Vec::with_capacity` allocation (OOM). The
    // per-chunk `pos` bounds checks below remain authoritative for correctness.
    const MIN_WIRE_CHUNK_LEN: usize = 4 + (1 + 4 + 1 + 24 + 4);
    let capacity_ceiling = (blob.len() - 4) / MIN_WIRE_CHUNK_LEN;
    let mut pos = 4usize;
    let mut chunks = Vec::with_capacity(count.min(capacity_ceiling));

    for _ in 0..count {
        if pos + 4 > blob.len() {
            return Err(ImageError::Decode("truncated blob (wire length)".into()));
        }
        let wire_len =
            u32::from_be_bytes([blob[pos], blob[pos + 1], blob[pos + 2], blob[pos + 3]]) as usize;
        pos += 4;

        if pos + wire_len > blob.len() {
            return Err(ImageError::Decode("truncated blob (wire data)".into()));
        }
        let wire = &blob[pos..pos + wire_len];
        pos += wire_len;

        // Parse wire format: [version:u8][index:u32][is_final:u8][nonce:24][len:u32][ciphertext]
        if wire.len() < 1 + 4 + 1 + 24 + 4 {
            return Err(ImageError::Decode("wire chunk too short".into()));
        }
        let version = wire[0];
        if version != CHUNK_FORMAT_VERSION {
            return Err(ImageError::Decode(format!(
                "unknown chunk version {version}"
            )));
        }
        let chunk_index = u32::from_be_bytes([wire[1], wire[2], wire[3], wire[4]]);
        let is_final = wire[5] != 0;
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&wire[6..30]);
        let ct_len = u32::from_be_bytes([wire[30], wire[31], wire[32], wire[33]]) as usize;
        if 34 + ct_len > wire.len() {
            return Err(ImageError::Decode("wire ciphertext truncated".into()));
        }
        let ciphertext = wire[34..34 + ct_len].to_vec();

        chunks.push(EncryptedChunk {
            chunk_index,
            is_final,
            nonce,
            ciphertext,
        });
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks};

    fn test_key() -> [u8; 32] {
        [0x11u8; 32]
    }

    fn test_file_id() -> [u8; 16] {
        [0xBBu8; 16]
    }

    /// Generate a valid 2x2 white PNG using the `image` crate.
    /// Using the crate itself avoids fragile hand-crafted byte arrays.
    fn minimal_png() -> Vec<u8> {
        use image::{ImageBuffer, Rgb};
        let img = ImageBuffer::from_fn(2, 2, |_, _| Rgb([255u8, 255u8, 255u8]));
        let dynamic = DynamicImage::ImageRgb8(img);
        encode_as_png(&dynamic).expect("encode test PNG should succeed")
    }

    #[test]
    fn decode_minimal_png_succeeds() {
        let png = minimal_png();
        let img = decode_clipboard_image(&png).expect("should decode minimal PNG");
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
    }

    #[test]
    fn empty_bytes_returns_unsupported() {
        let err = decode_clipboard_image(&[]).unwrap_err();
        assert!(matches!(err, ImageError::UnsupportedFormat));
    }

    #[test]
    fn random_bytes_return_decode_error() {
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
        let err = decode_clipboard_image(&garbage).unwrap_err();
        assert!(matches!(
            err,
            ImageError::Decode(_) | ImageError::UnsupportedFormat
        ));
    }

    #[test]
    fn encode_as_png_roundtrip() {
        // minimal_png() already produces valid PNG via encode_as_png, so decode it back
        let png = minimal_png();
        let img = decode_clipboard_image(&png).unwrap();
        let re_encoded = encode_as_png(&img).unwrap();
        // Re-encoded must itself be valid PNG with same dimensions
        let img2 = decode_clipboard_image(&re_encoded).unwrap();
        assert_eq!(img.width(), img2.width());
        assert_eq!(img.height(), img2.height());
    }

    #[test]
    fn full_encode_decode_pipeline_roundtrip() {
        let png = minimal_png();
        let key = test_key();
        let file_id = test_file_id();

        let (meta, chunks) = encode_image(&png, &key, &file_id, 0).expect("encode should succeed");
        assert_eq!(meta.width, 2);
        assert_eq!(meta.height, 2);
        assert_eq!(meta.original_size, png.len() as u64);
        assert!(meta.chunk_count >= 1);

        let recovered = decode_image(&chunks, &key, &file_id).expect("decode should succeed");
        // Recovered bytes should be valid PNG with same dimensions
        let img = decode_clipboard_image(&recovered).unwrap();
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
    }

    #[test]
    fn single_chunk_for_small_image() {
        let png = minimal_png();
        let key = test_key();
        let file_id = test_file_id();
        let (meta, chunks) = encode_image(&png, &key, &file_id, 0).unwrap();
        // A tiny image should fit in one chunk
        assert_eq!(chunks.len(), 1);
        assert_eq!(meta.chunk_count, 1);
        assert!(chunks[0].is_final);
    }

    #[test]
    fn multiple_chunks_for_large_data() {
        // Create a synthetic PNG that exceeds IMAGE_CHUNK_SIZE when re-encoded
        // We test chunking by passing artificially large raw data to encrypt_chunks directly
        let key = test_key();
        let file_id = test_file_id();
        // Generate data larger than one chunk
        let data = vec![0xABu8; IMAGE_CHUNK_SIZE + 100];
        let chunks = encrypt_chunks(&data, &key, &file_id, IMAGE_CHUNK_SIZE).unwrap();
        assert_eq!(chunks.len(), 2);
        let recovered = decrypt_chunks(&chunks, &key, &file_id).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn oversized_image_rejected() {
        let huge = vec![0u8; MAX_IMAGE_BYTES + 1];
        let key = test_key();
        let file_id = test_file_id();
        // max_bytes = 0 falls back to the library default MAX_IMAGE_BYTES.
        let err = encode_image(&huge, &key, &file_id, 0).unwrap_err();
        assert!(matches!(err, ImageError::TooLarge { .. }));
    }

    #[test]
    fn configured_cap_above_default_admits_larger_raw() {
        // Regression: a raw payload between the library default (10 MB) and the
        // user-configured cap (e.g. 25 MB default) must NOT be rejected when the
        // configured cap is threaded in. We can't feed 11 MB of decodable PNG
        // cheaply, so we assert the size gate itself: with a cap above the raw
        // length the gate passes (decode then fails on garbage with a *different*
        // error), whereas with the default cap it is rejected as TooLarge.
        let key = test_key();
        let file_id = test_file_id();
        let raw = vec![0u8; MAX_IMAGE_BYTES + 1]; // > 10 MB default, < 25 MB cap
        let configured_cap = 25 * 1024 * 1024;

        // Default cap (0 → MAX_IMAGE_BYTES): rejected by the size gate.
        let err = encode_image(&raw, &key, &file_id, 0).unwrap_err();
        assert!(matches!(err, ImageError::TooLarge { .. }));

        // Higher configured cap: the size gate is cleared, so the error (if any)
        // comes from decode/format, never TooLarge.
        let err = encode_image(&raw, &key, &file_id, configured_cap).unwrap_err();
        assert!(
            !matches!(err, ImageError::TooLarge { .. }),
            "raw under the configured cap must pass the size gate, got {err:?}"
        );
    }

    #[test]
    fn chunks_blob_serialisation_roundtrip() {
        let key = test_key();
        let file_id = test_file_id();
        let data = b"round-trip chunk blob test data";
        let chunks = encrypt_chunks(data, &key, &file_id, 10).unwrap();
        assert!(chunks.len() > 1);

        let blob = chunks_to_blob(&chunks).unwrap();
        let recovered_chunks = chunks_from_blob(&blob).unwrap();
        assert_eq!(recovered_chunks.len(), chunks.len());

        let plaintext = decrypt_chunks(&recovered_chunks, &key, &file_id).unwrap();
        assert_eq!(plaintext, data);
    }

    #[test]
    fn chunks_to_blob_presize_is_exact_and_byte_identical() {
        // F3: the pre-sized buffer must hold the blob with no realloc (capacity
        // == length) and produce byte-for-byte the same layout the unsized
        // version did. We reconstruct the expected bytes manually from the wire
        // encoding to prove the output is unchanged by the presize optimisation.
        let key = test_key();
        let file_id = test_file_id();
        let data = b"presize exactness across several chunks of clipboard data";
        let chunks = encrypt_chunks(data, &key, &file_id, 8).unwrap();
        assert!(chunks.len() > 1);

        let blob = chunks_to_blob(&chunks).unwrap();
        // No spare capacity: the presize was exact.
        assert_eq!(blob.capacity(), blob.len(), "presize must allocate exactly");

        // Rebuild the expected layout independently of the function under test.
        let mut expected = Vec::new();
        expected.extend_from_slice(&(chunks.len() as u32).to_be_bytes());
        for chunk in &chunks {
            let wire = chunk.to_wire();
            expected.extend_from_slice(&(wire.len() as u32).to_be_bytes());
            expected.extend_from_slice(&wire);
        }
        assert_eq!(blob, expected, "presized blob must be byte-identical");
    }

    #[test]
    fn blob_with_single_chunk_roundtrip() {
        let key = test_key();
        let file_id = test_file_id();
        let data = b"small";
        let chunks = encrypt_chunks(data, &key, &file_id, 64 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);

        let blob = chunks_to_blob(&chunks).unwrap();
        let recovered = chunks_from_blob(&blob).unwrap();
        let plaintext = decrypt_chunks(&recovered, &key, &file_id).unwrap();
        assert_eq!(plaintext, data);
    }

    #[test]
    fn truncated_blob_returns_error() {
        let key = test_key();
        let file_id = test_file_id();
        let chunks = encrypt_chunks(b"test", &key, &file_id, 64 * 1024).unwrap();
        let blob = chunks_to_blob(&chunks).unwrap();
        // Truncate to just the count field
        let truncated = &blob[..4];
        let err = chunks_from_blob(truncated).unwrap_err();
        assert!(matches!(err, ImageError::Decode(_)));
    }

    #[test]
    fn absurd_count_does_not_over_allocate() {
        // A corrupt/malicious blob declaring a u32::MAX chunk count but carrying
        // almost no actual data must NOT attempt a multi-GB pre-allocation.
        // It must fail with a bounded Decode error instead (the per-chunk bounds
        // checks reject the first chunk because there is no wire-length prefix).
        let mut blob = u32::MAX.to_be_bytes().to_vec(); // count = 4_294_967_295
        blob.extend_from_slice(&[0x00, 0x01]); // 2 trailing bytes, not even a full wire_len
        let err = chunks_from_blob(&blob).unwrap_err();
        assert!(matches!(err, ImageError::Decode(_)));
    }

    #[test]
    fn large_count_with_one_real_chunk_reads_safely() {
        // Build a valid single-chunk blob, then overwrite the count field with a
        // huge value. Deserialisation must still bound its allocation to the real
        // blob length and fail cleanly when it runs past the available bytes,
        // rather than reserving capacity for the bogus count.
        let key = test_key();
        let file_id = test_file_id();
        let chunks = encrypt_chunks(b"hi", &key, &file_id, 64 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        let mut blob = chunks_to_blob(&chunks).unwrap();
        blob[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
        // Reading the (single) real chunk succeeds, then the second iteration
        // hits the wire-length bounds check and errors — no huge allocation.
        let err = chunks_from_blob(&blob).unwrap_err();
        assert!(matches!(err, ImageError::Decode(_)));
    }

    /// Regression guard for the buffer-lifetime refactor: the encode pipeline
    /// must remain byte-identical to its decoded-PNG plaintext. Encrypting then
    /// decrypting the chunks must reproduce exactly the PNG bytes that
    /// `encode_as_png` yields for the decoded image. Scoping/dropping the
    /// decoded `DynamicImage` and the intermediate PNG buffer earlier must not
    /// change *what* is produced, only *when* it is freed.
    #[test]
    fn encode_chunks_reproduce_canonical_png_bytes() {
        let key = test_key();
        let file_id = test_file_id();
        // A non-trivial image so the PNG buffer is meaningfully sized.
        let raw = synthetic_png(300, 200);

        // Canonical PNG bytes the pipeline must encrypt: decode then re-encode.
        let decoded = decode_clipboard_image(&raw).unwrap();
        let canonical_png = encode_as_png(&decoded).unwrap();

        let (meta, chunks) = encode_image(&raw, &key, &file_id, 0).unwrap();
        assert_eq!(meta.width, 300);
        assert_eq!(meta.height, 200);
        assert_eq!(meta.chunk_count as usize, chunks.len());

        let recovered = decrypt_chunks(&chunks, &key, &file_id).unwrap();
        assert_eq!(
            recovered, canonical_png,
            "encrypted chunks must decrypt to the exact canonical PNG bytes"
        );
    }

    #[test]
    fn wrong_key_fails_decode() {
        let key = test_key();
        let bad_key = [0xFFu8; 32];
        let file_id = test_file_id();
        let png = minimal_png();
        let (_, chunks) = encode_image(&png, &key, &file_id, 0).unwrap();
        let err = decode_image(&chunks, &bad_key, &file_id).unwrap_err();
        assert!(matches!(err, ImageError::Chunk(_)));
    }

    // --- Wave 3.4: thumbnail helper ---

    /// Build a synthetic RGB image of `(w, h)` and return its PNG bytes.
    fn synthetic_png(w: u32, h: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgb};
        let img = ImageBuffer::from_fn(w, h, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 128u8]));
        encode_as_png(&DynamicImage::ImageRgb8(img)).expect("encode synthetic PNG")
    }

    #[test]
    fn thumbnail_downscales_large_image_preserving_aspect() {
        // 1000x500 → bounded to 200x150 keeps the 2:1 aspect → 200x100
        let png = synthetic_png(1000, 500);
        let (bytes, w, h) = thumbnail(&png, 200, 150).expect("thumbnail must succeed");
        assert!(w <= 200 && h <= 150, "thumb {}x{} must fit bounds", w, h);
        assert_eq!(w, 200, "longest side must hit max_w for 2:1 source");
        assert_eq!(h, 100, "aspect ratio must be preserved");
        assert_eq!(
            bytes.len() as u32,
            w * h * 4,
            "RGBA8 byte count must match dimensions"
        );
    }

    #[test]
    fn thumbnail_no_op_for_small_image_returns_original_dimensions() {
        // 64x32 fits within 200x150 → no resize, but still RGBA8 output.
        let png = synthetic_png(64, 32);
        let (bytes, w, h) = thumbnail(&png, 200, 150).expect("thumbnail must succeed");
        assert_eq!(w, 64);
        assert_eq!(h, 32);
        assert_eq!(bytes.len() as u32, 64 * 32 * 4);
    }

    #[test]
    fn thumbnail_rejects_zero_bounds() {
        let png = synthetic_png(10, 10);
        assert!(matches!(
            thumbnail(&png, 0, 100).unwrap_err(),
            ImageError::Decode(_)
        ));
        assert!(matches!(
            thumbnail(&png, 100, 0).unwrap_err(),
            ImageError::Decode(_)
        ));
    }

    #[test]
    fn thumbnail_rejects_garbage_bytes() {
        let err = thumbnail(&[0xDE, 0xAD, 0xBE, 0xEF], 100, 100).unwrap_err();
        assert!(matches!(
            err,
            ImageError::Decode(_) | ImageError::UnsupportedFormat
        ));
    }

    // --- Variant B Phase 1: capture-time encrypted thumbnail ---

    fn test_thumb_file_id() -> [u8; 16] {
        [0xCCu8; 16]
    }

    #[test]
    fn thumbnail_encrypt_roundtrip() {
        let png = synthetic_png(400, 200);
        let key = test_key();
        let thumb_id = test_thumb_file_id();
        let img = decode_clipboard_image(&png).unwrap();

        let blob = encode_thumbnail(&img, &key, &thumb_id, THUMBNAIL_MAX_DIM).unwrap();
        let recovered = decode_thumbnail(&blob, &key, &thumb_id).unwrap();
        // Recovered bytes must be a valid image decodable back to a thumbnail
        // bounded by THUMBNAIL_MAX_DIM.
        let thumb_img = decode_clipboard_image(&recovered).unwrap();
        assert!(thumb_img.width() <= THUMBNAIL_MAX_DIM);
        assert!(thumb_img.height() <= THUMBNAIL_MAX_DIM);
    }

    #[test]
    fn thumbnail_is_smaller_or_equal_dimensions_than_original() {
        // 1000x500 source bounded to 192 → longest side hits 192, aspect kept.
        let png = synthetic_png(1000, 500);
        let key = test_key();
        let thumb_id = test_thumb_file_id();
        let img = decode_clipboard_image(&png).unwrap();
        let (orig_w, orig_h) = img.dimensions();

        let blob = encode_thumbnail(&img, &key, &thumb_id, THUMBNAIL_MAX_DIM).unwrap();
        let thumb =
            decode_clipboard_image(&decode_thumbnail(&blob, &key, &thumb_id).unwrap()).unwrap();
        assert!(
            thumb.width() <= orig_w && thumb.height() <= orig_h,
            "thumb {}x{} must not exceed original {}x{}",
            thumb.width(),
            thumb.height(),
            orig_w,
            orig_h
        );
        assert_eq!(thumb.width(), 192, "longest side must hit the 192 bound");
        assert_eq!(thumb.height(), 96, "aspect ratio (2:1) must be preserved");
    }

    #[test]
    fn thumbnail_never_upscales_small_source() {
        // 64x32 source is already under the cap → thumbnail keeps its dimensions.
        let png = synthetic_png(64, 32);
        let key = test_key();
        let thumb_id = test_thumb_file_id();
        let img = decode_clipboard_image(&png).unwrap();

        let blob = encode_thumbnail(&img, &key, &thumb_id, THUMBNAIL_MAX_DIM).unwrap();
        let thumb =
            decode_clipboard_image(&decode_thumbnail(&blob, &key, &thumb_id).unwrap()).unwrap();
        assert_eq!(thumb.width(), 64, "must not upscale width");
        assert_eq!(thumb.height(), 32, "must not upscale height");
    }

    #[test]
    fn thumb_dims_exceed_cap_flags_oversized_only() {
        // A thumbnail stored under the old 680 cap exceeds the new 192 cap.
        assert!(
            thumb_dims_exceed_cap(680, 340),
            "680 longest side > 192 cap"
        );
        assert!(thumb_dims_exceed_cap(340, 680), "tall variant also exceeds");
        // Exactly at the cap is conformant (not regenerated).
        assert!(!thumb_dims_exceed_cap(192, 96), "192 == cap is conformant");
        assert!(!thumb_dims_exceed_cap(96, 192), "tall-at-cap is conformant");
        // Smaller than the cap is conformant.
        assert!(!thumb_dims_exceed_cap(64, 32), "under cap is conformant");
        // Zero dims (missing/corrupt meta) → never flagged.
        assert!(!thumb_dims_exceed_cap(0, 0), "zero dims are not flagged");
        assert!(!thumb_dims_exceed_cap(0, 800), "partial-zero not flagged");
    }

    #[test]
    fn thumbnail_wrong_file_id_fails_decode() {
        // AAD isolation: decrypting the thumbnail with a different thumb_file_id
        // must fail the chunk integrity check.
        let png = synthetic_png(400, 200);
        let key = test_key();
        let thumb_id = test_thumb_file_id();
        let img = decode_clipboard_image(&png).unwrap();

        let blob = encode_thumbnail(&img, &key, &thumb_id, THUMBNAIL_MAX_DIM).unwrap();
        let wrong_id = [0x00u8; 16];
        let err = decode_thumbnail(&blob, &key, &wrong_id).unwrap_err();
        assert!(matches!(err, ImageError::Chunk(_)));
    }

    #[test]
    fn encode_image_full_produces_full_and_thumbnail_from_one_decode() {
        let png = synthetic_png(1000, 500);
        let key = test_key();
        let file_id = test_file_id();
        let thumb_id = test_thumb_file_id();

        let (meta, chunks, thumb_blob, thumb_w, thumb_h) = encode_image_full(
            &png,
            &key,
            &file_id,
            &thumb_id,
            0,
            crate::config::MAX_DECODED_IMAGE_MB,
            THUMBNAIL_MAX_DIM,
        )
        .unwrap();

        // Full image meta reflects the ORIGINAL dimensions.
        assert_eq!(meta.width, 1000);
        assert_eq!(meta.height, 500);
        assert_eq!(meta.chunk_count as usize, chunks.len());
        assert!(meta.chunk_count >= 1);

        // Thumbnail is the downscaled 2:1 → 192x96.
        assert_eq!(thumb_w, 192);
        assert_eq!(thumb_h, 96);

        // Full blob decodes (via file_id) to the original dimensions.
        let full = decode_clipboard_image(&decode_image(&chunks, &key, &file_id).unwrap()).unwrap();
        assert_eq!(full.dimensions(), (1000, 500));

        // Thumb blob decodes (via thumb_file_id) to the thumbnail dimensions.
        let thumb =
            decode_clipboard_image(&decode_thumbnail(&thumb_blob, &key, &thumb_id).unwrap())
                .unwrap();
        assert_eq!(thumb.dimensions(), (192, 96));
    }

    #[test]
    fn encode_image_full_isolates_full_and_thumb_aad() {
        // The full file_id must NOT decrypt the thumbnail blob and vice versa.
        let png = synthetic_png(400, 200);
        let key = test_key();
        let file_id = test_file_id();
        let thumb_id = test_thumb_file_id();

        let (_, chunks, thumb_blob, _, _) = encode_image_full(
            &png,
            &key,
            &file_id,
            &thumb_id,
            0,
            crate::config::MAX_DECODED_IMAGE_MB,
            THUMBNAIL_MAX_DIM,
        )
        .unwrap();

        // Cross-decrypt must fail (distinct AAD).
        assert!(decode_thumbnail(&thumb_blob, &key, &file_id).is_err());
        assert!(decode_image(&chunks, &key, &thumb_id).is_err());
    }

    #[test]
    fn encode_image_full_rejects_oversized_raw() {
        let huge = vec![0u8; MAX_IMAGE_BYTES + 1];
        let key = test_key();
        let file_id = test_file_id();
        let thumb_id = test_thumb_file_id();
        let err = encode_image_full(
            &huge,
            &key,
            &file_id,
            &thumb_id,
            0,
            crate::config::MAX_DECODED_IMAGE_MB,
            THUMBNAIL_MAX_DIM,
        )
        .unwrap_err();
        assert!(matches!(err, ImageError::TooLarge { .. }));
    }

    #[test]
    fn wrong_file_id_fails_decode() {
        let key = test_key();
        let file_id = test_file_id();
        let bad_file_id = [0x00u8; 16];
        let png = minimal_png();
        let (_, chunks) = encode_image(&png, &key, &file_id, 0).unwrap();
        let err = decode_image(&chunks, &key, &bad_file_id).unwrap_err();
        assert!(matches!(err, ImageError::Chunk(_)));
    }

    // --- Decode-bomb / OOM DoS prevention ---

    /// A 66-byte PNG with a valid IHDR declaring 30000×30000 pixels (RGB8).
    ///
    /// At 3 bytes/pixel the uncompressed pixel buffer would be ~2.7 GB.
    /// The file contains only a stub IDAT so it fits in 66 bytes total.
    /// CRCs were pre-computed with Python's `zlib.crc32`.
    ///
    /// Structure: PNG signature (8) + IHDR chunk (25) + IDAT chunk (21) + IEND (12)
    #[rustfmt::skip]
    const DECODE_BOMB_PNG: &[u8] = &[
        // PNG signature
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        // IHDR chunk: length=13, type="IHDR"
        0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52,
        // width=30000 (0x00007530), height=30000
        0x00, 0x00, 0x75, 0x30,
        0x00, 0x00, 0x75, 0x30,
        // bit_depth=8, color_type=2 (RGB), compression=0, filter=0, interlace=0
        0x08, 0x02, 0x00, 0x00, 0x00,
        // CRC of IHDR type+data
        0xE9, 0x45, 0x6F, 0xED,
        // IDAT chunk: length=9, type="IDAT", stub deflate stream, CRC
        0x00, 0x00, 0x00, 0x09,
        0x49, 0x44, 0x41, 0x54,
        0x78, 0x9C, 0x62, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01,
        0xFA, 0xCE, 0xC8, 0x14,
        // IEND chunk
        0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44,
        0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn decode_bomb_png_rejected_before_oom() {
        // With a 1 MB alloc cap the 30000×30000×3 ≈ 2.7 GB pixel buffer must be
        // rejected by image::Limits before any large allocation occurs.
        let err = decode_clipboard_image_limited(DECODE_BOMB_PNG, 1)
            .expect_err("decode-bomb must be rejected by Limits");
        // Must be a Decode error (from image::Limits), never a panic or OOM.
        assert!(
            matches!(err, ImageError::Decode(_) | ImageError::UnsupportedFormat),
            "expected Decode or UnsupportedFormat, got: {err:?}"
        );
    }
}
