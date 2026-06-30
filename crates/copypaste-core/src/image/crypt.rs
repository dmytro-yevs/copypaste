//! AEAD chunk encrypt/decrypt pipelines for full-resolution images and thumbnails.
//!
//! All functions here operate on `EncryptedChunk` slices keyed by a `file_id` or
//! `thumb_file_id` that is bound as AEAD AAD, ensuring full images and their
//! thumbnails cannot be cross-decrypted even with the same key.

use image::DynamicImage;

use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};

use super::blob::{chunks_from_blob, chunks_to_blob};
use super::codec::{decode_clipboard_image_limited, encode_as_png};
use super::{ImageError, ImageMeta, IMAGE_CHUNK_SIZE, MAX_IMAGE_BYTES};

/// Downscale `img` to fit `(max_dim, max_dim)` (never upscaling) and encode the
/// result as PNG bytes. Returns `(png_bytes, width, height)` of the thumbnail.
///
/// Shared by [`encode_thumbnail`] and [`encode_image_full`] so the thumbnail
/// dimensions can be recorded in the image meta without re-deriving them.
pub(super) fn encode_thumbnail_bytes(
    img: &DynamicImage,
    max_dim: u32,
) -> Result<(Vec<u8>, u32, u32), ImageError> {
    use image::GenericImageView;
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

/// Full encode pipeline:
///   raw clipboard bytes → decode → PNG → split into chunks → encrypt
///
/// `max_bytes` is the configured raw-byte ceiling (the daemon threads
/// `AppConfig::max_image_size_bytes` here). Passing `0` falls back to the
/// library default [`MAX_IMAGE_BYTES`] so callers without config still get a
/// sane bound.
///
/// Enforces the `make_limits` allocation cap (derived from
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
///
/// For callers that process the bytes before use, prefer [`decode_image_zeroizing`]
/// which wraps the plaintext in a `Zeroizing<Vec<u8>>` that scrubs the heap on drop.
pub fn decode_image(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<Vec<u8>, ImageError> {
    let png_bytes = decrypt_chunks(chunks, key, file_id)?;
    Ok(png_bytes)
}

/// Full decode pipeline returning a `Zeroizing<Vec<u8>>` so the plaintext PNG
/// bytes are scrubbed from the heap when the caller drops the buffer.
///
/// Prefer this variant when the caller processes the bytes before writing to
/// NSPasteboard: the `Zeroizing` wrapper ensures no plaintext lingers in freed
/// memory regardless of what intermediate code is added in the future.
///
/// CopyPaste-dgqm: pre-wired so any future expansion of the decode path can
/// use this variant and automatically inherit the zeroize-on-drop contract.
pub fn decode_image_zeroizing(
    chunks: &[EncryptedChunk],
    key: &[u8; 32],
    file_id: &[u8; 16],
) -> Result<zeroize::Zeroizing<Vec<u8>>, ImageError> {
    let png_bytes = decrypt_chunks(chunks, key, file_id)?;
    Ok(zeroize::Zeroizing::new(png_bytes))
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

/// Decode an encrypted thumbnail blob (produced by [`encode_thumbnail`]) back
/// into the encoded image bytes (PNG). Mirrors [`decode_image`].
///
/// `thumb_file_id` MUST be the same value passed to [`encode_thumbnail`]; the
/// chunk AEAD binds it as AAD, so a wrong id fails the integrity check rather
/// than returning garbage.
///
/// For callers that process the bytes before rendering, prefer
/// [`decode_thumbnail_zeroizing`] which scrubs the heap on drop.
pub fn decode_thumbnail(
    blob: &[u8],
    key: &[u8; 32],
    thumb_file_id: &[u8; 16],
) -> Result<Vec<u8>, ImageError> {
    let chunks = chunks_from_blob(blob)?;
    let bytes = decrypt_chunks(&chunks, key, thumb_file_id)?;
    Ok(bytes)
}

/// Like [`decode_thumbnail`] but wraps the plaintext in `Zeroizing<Vec<u8>>`
/// so the decrypted thumbnail bytes are scrubbed from the heap on drop.
///
/// CopyPaste-dgqm: pre-wired for future callers that hold the decrypted bytes
/// in a processing pipeline before passing them to the render layer.
pub fn decode_thumbnail_zeroizing(
    blob: &[u8],
    key: &[u8; 32],
    thumb_file_id: &[u8; 16],
) -> Result<zeroize::Zeroizing<Vec<u8>>, ImageError> {
    let chunks = chunks_from_blob(blob)?;
    let bytes = decrypt_chunks(&chunks, key, thumb_file_id)?;
    Ok(zeroize::Zeroizing::new(bytes))
}

/// Lazy-backfill path: decode raw PNG bytes, then produce an encrypted
/// thumbnail blob identical in format to the capture-time path.
///
/// This is the Phase-4 helper used by `get_item_thumbnail` when a legacy
/// image item has `thumb IS NULL`. The caller decrypts the full-res content
/// to `png_bytes` first (via [`decode_image`]), then passes them here.
///
/// Pipeline:
///   `png_bytes` → [`crate::decode_clipboard_image`] → [`encode_thumbnail`]
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
    use super::codec::decode_clipboard_image;
    let img = decode_clipboard_image(png_bytes)?;
    let (bytes, w, h) = encode_thumbnail_bytes(&img, max_dim)?;
    let chunks = encrypt_chunks(&bytes, key, thumb_file_id, IMAGE_CHUNK_SIZE)?;
    let blob = chunks_to_blob(&chunks)?;
    Ok((blob, w, h))
}

/// True when a *stored* thumbnail whose recorded dimensions are
/// `(thumb_w, thumb_h)` is larger than the current [`super::THUMBNAIL_MAX_DIM`] cap on
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
    thumb_w.max(thumb_h) > super::THUMBNAIL_MAX_DIM
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

    // [PERF] Decode once and scope the large DynamicImage to the smallest
    // possible block — it is dropped as soon as we have extracted:
    //   (a) the dimensions (width, height) — two u32s
    //   (b) the thumbnail bytes (encode_thumbnail_bytes downscales first, so
    //       the intermediate thumb DynamicImage is only ~192×192 pixels)
    //   (c) the PNG bytes (encode_as_png re-encodes the full-res image)
    //
    // After this block, `img` is freed. The expensive chunk encryption that
    // follows operates on `png_bytes` and `thumb_bytes` only — the large
    // decoded DynamicImage is no longer in memory. For a 4K image (~53 MB
    // decoded) this cuts the peak RSS during encrypt_chunks by ~53 MB.
    //
    // Ordering within the scope: thumbnail first, then full PNG. This means
    // both png_bytes (compressed, small) AND thumb_bytes (tiny) are live
    // while img is still alive, but img is freed before the chunk encryption
    // calls — the peak is raw + img + png_bytes + thumb_bytes rather than
    // raw + img + png_bytes + thumb_bytes + all_chunk_ciphertexts.
    let (width, height, png_bytes, thumb_bytes, thumb_w, thumb_h) = {
        let img = decode_clipboard_image_limited(raw, max_decoded_mb)?;
        let (width, height) = (img.width(), img.height());
        // Thumbnail bytes are small (max 192×192 PNG) — generate first so the
        // thumbnail downscale DynamicImage and img can both be freed together
        // at the end of this scope rather than surviving into encrypt_chunks.
        let (thumb_bytes, thumb_w, thumb_h) = encode_thumbnail_bytes(&img, thumb_max_dim)?;
        // Full-resolution PNG re-encode. `img` is still alive here (needed
        // for encode_as_png), but will be dropped at the end of this block.
        let png_bytes = encode_as_png(&img)?;
        // `img` is dropped here — before chunk encryption begins.
        (width, height, png_bytes, thumb_bytes, thumb_w, thumb_h)
    };
    let original_size = raw.len() as u64;

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
