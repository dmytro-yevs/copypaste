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

use image::{DynamicImage, GenericImageView, ImageFormat};
use std::io::Cursor;
use thiserror::Error;

use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};

/// 512 KB chunk size.
pub const IMAGE_CHUNK_SIZE: usize = 512 * 1024;
/// Maximum accepted image size (raw bytes before compression): 10 MB.
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

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

/// Decode raw clipboard bytes (PNG or TIFF) into a `DynamicImage`.
///
/// Tries PNG first, then TIFF. Returns `ImageError::UnsupportedFormat` if
/// neither decode succeeds.
pub fn decode_clipboard_image(raw: &[u8]) -> Result<DynamicImage, ImageError> {
    if raw.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    // Try PNG
    if let Ok(img) = image::load_from_memory_with_format(raw, ImageFormat::Png) {
        return Ok(img);
    }

    // Try TIFF (macOS often puts TIFF on pasteboard)
    if let Ok(img) = image::load_from_memory_with_format(raw, ImageFormat::Tiff) {
        return Ok(img);
    }

    // Generic sniff (handles BMP, etc.)
    image::load_from_memory(raw).map_err(|e| ImageError::Decode(e.to_string()))
}

/// Decode raw image bytes (PNG/TIFF/BMP) and produce an RGBA8 thumbnail
/// that fits within `(max_w, max_h)`, preserving aspect ratio.
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
/// Returns `(ImageMeta, Vec<EncryptedChunk>)`.
pub fn encode_image(
    raw: &[u8],
    key: &[u8; 32],
    file_id: &[u8; 16],
    max_bytes: usize,
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

    let img = decode_clipboard_image(raw)?;
    let (width, height) = (img.width(), img.height());
    let original_size = raw.len() as u64;

    let png_bytes = encode_as_png(&img)?;

    let chunks = encrypt_chunks(&png_bytes, key, file_id, IMAGE_CHUNK_SIZE)?;
    let chunk_count = chunks.len() as u32;

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

/// Serialize chunks to a flat byte blob for SQLite BLOB storage.
///
/// Format: `[chunk_count: u32 BE] [chunk_0_wire] [chunk_1_wire] ...`
pub fn chunks_to_blob(chunks: &[EncryptedChunk]) -> Vec<u8> {
    let mut out = Vec::new();
    let count = chunks.len() as u32;
    out.extend_from_slice(&count.to_be_bytes());
    for chunk in chunks {
        let wire = chunk.to_wire();
        out.extend_from_slice(&(wire.len() as u32).to_be_bytes());
        out.extend_from_slice(&wire);
    }
    out
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

        let blob = chunks_to_blob(&chunks);
        let recovered_chunks = chunks_from_blob(&blob).unwrap();
        assert_eq!(recovered_chunks.len(), chunks.len());

        let plaintext = decrypt_chunks(&recovered_chunks, &key, &file_id).unwrap();
        assert_eq!(plaintext, data);
    }

    #[test]
    fn blob_with_single_chunk_roundtrip() {
        let key = test_key();
        let file_id = test_file_id();
        let data = b"small";
        let chunks = encrypt_chunks(data, &key, &file_id, 64 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);

        let blob = chunks_to_blob(&chunks);
        let recovered = chunks_from_blob(&blob).unwrap();
        let plaintext = decrypt_chunks(&recovered, &key, &file_id).unwrap();
        assert_eq!(plaintext, data);
    }

    #[test]
    fn truncated_blob_returns_error() {
        let key = test_key();
        let file_id = test_file_id();
        let chunks = encrypt_chunks(b"test", &key, &file_id, 64 * 1024).unwrap();
        let blob = chunks_to_blob(&chunks);
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
        let mut blob = chunks_to_blob(&chunks);
        blob[0..4].copy_from_slice(&u32::MAX.to_be_bytes());
        // Reading the (single) real chunk succeeds, then the second iteration
        // hits the wire-length bounds check and errors — no huge allocation.
        let err = chunks_from_blob(&blob).unwrap_err();
        assert!(matches!(err, ImageError::Decode(_)));
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
}
