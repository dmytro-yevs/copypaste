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

mod blob;
mod codec;
mod crypt;

use thiserror::Error;

use crate::crypto::chunks::ChunkError;

// Re-export the public surface so callers can use `crate::image::*` unchanged.
pub use blob::{chunks_from_blob, chunks_to_blob};
pub use codec::{decode_clipboard_image, decode_clipboard_image_limited, encode_as_png, thumbnail};
pub use crypt::{
    decode_image, decode_image_zeroizing, decode_thumbnail, decode_thumbnail_zeroizing,
    encode_image, encode_image_full, encode_image_with_limit, encode_thumbnail,
    encode_thumbnail_from_png, thumb_dims_exceed_cap,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks};
    use image::{DynamicImage, GenericImageView};

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

    // CopyPaste-dgqm: Zeroizing export path tests
    // These verify that decode_image_zeroizing and decode_thumbnail_zeroizing
    // return the same plaintext as their non-zeroizing counterparts.

    #[test]
    fn decode_image_zeroizing_matches_decode_image() {
        let key = test_key();
        let file_id = test_file_id();
        let png = minimal_png();

        let (_, chunks) = encode_image(&png, &key, &file_id, 0).unwrap();

        let plain = decode_image(&chunks, &key, &file_id).unwrap();
        let zeroizing = decode_image_zeroizing(&chunks, &key, &file_id).unwrap();

        // Both must produce identical bytes — the Zeroizing wrapper is transparent.
        assert_eq!(
            *zeroizing, plain,
            "decode_image_zeroizing must return the same bytes as decode_image"
        );
    }

    #[test]
    fn decode_thumbnail_zeroizing_matches_decode_thumbnail() {
        let key = test_key();
        let thumb_file_id = [0xCCu8; 16];
        let png = minimal_png();
        let img = decode_clipboard_image(&png).unwrap();

        let blob = encode_thumbnail(&img, &key, &thumb_file_id, THUMBNAIL_MAX_DIM).unwrap();

        let plain = decode_thumbnail(&blob, &key, &thumb_file_id).unwrap();
        let zeroizing = decode_thumbnail_zeroizing(&blob, &key, &thumb_file_id).unwrap();

        assert_eq!(
            *zeroizing, plain,
            "decode_thumbnail_zeroizing must return the same bytes as decode_thumbnail"
        );
    }

    #[test]
    fn decode_image_zeroizing_wrong_key_fails() {
        let key = test_key();
        let bad_key = [0xFFu8; 32];
        let file_id = test_file_id();
        let png = minimal_png();

        let (_, chunks) = encode_image(&png, &key, &file_id, 0).unwrap();

        let err = decode_image_zeroizing(&chunks, &bad_key, &file_id).unwrap_err();
        assert!(
            matches!(err, ImageError::Chunk(_)),
            "wrong key must fail AEAD auth on the Zeroizing path too"
        );
    }
}
