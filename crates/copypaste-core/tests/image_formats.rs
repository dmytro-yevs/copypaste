//! Beta-bonus: image-format round-trip + thumbnail invariant suite.
//!
//! These tests live as an **integration test** (not in `src/image.rs#tests`)
//! so they exercise the public `copypaste_core::image` API exactly as
//! downstream crates (daemon, UI) do.
//!
//! Coverage:
//!   - PNG / JPEG decode + thumbnail aspect preservation
//!   - WebP behaviour gated by build feature (currently `#[ignore]` — the
//!     production `image` dep is pinned to `["png", "tiff"]` and WebP needs
//!     the `webp` feature; flip with `cargo test --features webp` once
//!     enabled workspace-wide).
//!   - Animated GIF returns ONLY frame 0
//!   - Corrupted headers must `Err`, never panic
//!   - Huge nominal dimensions must reject / scale safely (no allocator OOM)
//!   - Aspect ratio is never inverted (landscape stays landscape)
//!
//! NOTE: All fixtures are synthesised in-process via the `image` crate —
//! no binary blobs committed to the repo.

use copypaste_core::image::{decode_clipboard_image, thumbnail, ImageError, MAX_IMAGE_BYTES};
use image::{
    codecs::gif::{GifEncoder, Repeat},
    codecs::jpeg::JpegEncoder,
    DynamicImage, Frame, ImageBuffer, Rgb, Rgba, RgbaImage,
};
use std::io::Cursor;

// ----------------------------- helpers ----------------------------------

fn synth_rgb_png(w: u32, h: u32) -> Vec<u8> {
    let img = ImageBuffer::from_fn(w, h, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 64]));
    let dyn_img = DynamicImage::ImageRgb8(img);
    let mut buf = Cursor::new(Vec::new());
    dyn_img
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encode");
    buf.into_inner()
}

fn synth_jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = ImageBuffer::from_fn(w, h, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 200]));
    let dyn_img = DynamicImage::ImageRgb8(img);
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut out, 85);
    encoder
        .encode_image(&dyn_img)
        .expect("JPEG encode must succeed");
    out
}

/// Build a 2-frame animated GIF (8x8). Frame 0 is solid RED, frame 1 is
/// solid BLUE — letting us assert that only frame 0 is decoded.
fn synth_animated_gif() -> Vec<u8> {
    let mut frame0: RgbaImage = ImageBuffer::new(8, 8);
    for px in frame0.pixels_mut() {
        *px = Rgba([255, 0, 0, 255]);
    }
    let mut frame1: RgbaImage = ImageBuffer::new(8, 8);
    for px in frame1.pixels_mut() {
        *px = Rgba([0, 0, 255, 255]);
    }

    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder.set_repeat(Repeat::Infinite).expect("set repeat");
        encoder
            .encode_frame(Frame::new(frame0))
            .expect("encode frame 0");
        encoder
            .encode_frame(Frame::new(frame1))
            .expect("encode frame 1");
    }
    out
}

// ------------------------------- tests ----------------------------------

#[test]
fn png_decode_and_thumbnail_preserves_aspect() {
    // 800x400 (2:1) → bounded by (200, 200) → expect 200x100
    let png = synth_rgb_png(800, 400);

    let img = decode_clipboard_image(&png).expect("PNG must decode");
    assert_eq!(img.width(), 800);
    assert_eq!(img.height(), 400);

    let (bytes, w, h) = thumbnail(&png, 200, 200).expect("thumbnail PNG");
    assert!(w <= 200 && h <= 200, "thumb must fit bounds, got {w}x{h}");
    assert_eq!(w, 200, "longest side hits max for 2:1 source");
    assert_eq!(h, 100, "aspect 2:1 preserved");
    assert_eq!(bytes.len() as u32, w * h * 4, "RGBA8 byte count");
}

#[test]
fn jpeg_decode_and_thumbnail() {
    let jpeg = synth_jpeg(400, 200);

    let img = decode_clipboard_image(&jpeg).expect("JPEG must decode (test-build feature)");
    // JPEG is lossy + can pad dims by ±1 on some encoders, so allow tolerance.
    assert!(
        (img.width() as i64 - 400).abs() <= 2,
        "JPEG width within tolerance, got {}",
        img.width()
    );
    assert!(
        (img.height() as i64 - 200).abs() <= 2,
        "JPEG height within tolerance, got {}",
        img.height()
    );

    let (bytes, w, h) = thumbnail(&jpeg, 100, 100).expect("thumbnail JPEG");
    assert!(w <= 100 && h <= 100, "thumb must fit bounds, got {w}x{h}");
    // 2:1 → 100x50 (±1 from JPEG rounding upstream)
    assert!((w as i64 - 100).abs() <= 2);
    assert!((h as i64 - 50).abs() <= 2);
    assert_eq!(bytes.len() as u32, w * h * 4);
}

/// WebP is NOT enabled in the workspace `image` feature set right now
/// (production deps are `["png", "tiff"]`, test deps add `["jpeg", "gif"]`).
/// This test stays ignored until WebP support is intentionally turned on
/// — at which point flip the `#[ignore]` and remove the `_marker` arg.
#[test]
#[ignore = "WebP requires `webp` feature on the `image` crate — not enabled in workspace pin"]
fn webp_decode_if_supported() {
    // Placeholder: when enabled, synthesise via `image::codecs::webp::WebPEncoder`
    // and call decode_clipboard_image, asserting dims round-trip.
}

#[test]
fn gif_animated_first_frame_only() {
    let gif = synth_animated_gif();

    // `decode_clipboard_image` uses `load_from_memory*`, which decodes ONLY
    // the first frame of an animated GIF — verify by sampling pixel (0,0)
    // and asserting it's red (frame 0), not blue (frame 1).
    let img = decode_clipboard_image(&gif).expect("animated GIF must decode frame 0");
    assert_eq!(img.width(), 8);
    assert_eq!(img.height(), 8);

    let rgba = img.to_rgba8();
    let p = rgba.get_pixel(0, 0);
    assert_eq!(
        p.0[0], 255,
        "frame 0 pixel must be RED — animated GIF leaked another frame? got {:?}",
        p.0
    );
    assert_eq!(p.0[2], 0, "frame 0 blue channel must be 0, got {:?}", p.0);
}

#[test]
fn corrupted_header_returns_error_not_panic() {
    // 8 bytes that look like a PNG signature start (\x89PNG) but truncated.
    let almost_png: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let err = decode_clipboard_image(almost_png).expect_err("truncated PNG must err");
    assert!(
        matches!(err, ImageError::Decode(_) | ImageError::UnsupportedFormat),
        "unexpected error kind: {err:?}"
    );

    // Pure garbage
    let garbage: Vec<u8> = (0..256).map(|i| (i ^ 0x5A) as u8).collect();
    let err = decode_clipboard_image(&garbage).expect_err("garbage bytes must err");
    assert!(matches!(
        err,
        ImageError::Decode(_) | ImageError::UnsupportedFormat
    ));

    // Empty
    let err = decode_clipboard_image(&[]).expect_err("empty must err");
    assert!(matches!(err, ImageError::UnsupportedFormat));
}

/// Synthesising a real 10000x10000 image (~400 MB RGBA) would OOM CI.
/// Instead we assert the encode pipeline rejects oversized RAW byte
/// payloads (≥ MAX_IMAGE_BYTES) and that thumbnail safely refuses zero
/// bounds — covering the two clamp paths the production code actually has.
#[test]
fn max_dimension_clamp() {
    // Path A: huge raw byte payload is rejected before decode allocates.
    let huge = vec![0u8; MAX_IMAGE_BYTES + 1];
    let err = copypaste_core::image::encode_image(&huge, &[0u8; 32], &[0u8; 16])
        .expect_err("oversize must err");
    assert!(matches!(err, ImageError::TooLarge { .. }));

    // Path B: zero-bound thumbnail request is rejected (would otherwise
    // panic inside the `image` crate's resize math).
    let png = synth_rgb_png(64, 64);
    assert!(matches!(
        thumbnail(&png, 0, 100).unwrap_err(),
        ImageError::Decode(_)
    ));
    assert!(matches!(
        thumbnail(&png, 100, 0).unwrap_err(),
        ImageError::Decode(_)
    ));

    // Path C: a moderately wide source (4096x32) clamped to a tiny box
    // must still produce w<=bound, h<=bound, h>=1.
    let wide = synth_rgb_png(4096, 32);
    let (_, w, h) = thumbnail(&wide, 64, 64).expect("wide thumb");
    assert!(w <= 64 && h <= 64 && h >= 1, "got {w}x{h}");
}

#[test]
fn aspect_ratio_invariant() {
    // Landscape 1000x500 must stay landscape after thumbnailing.
    let png = synth_rgb_png(1000, 500);
    let (_, w, h) = thumbnail(&png, 300, 300).expect("landscape thumb");
    assert!(w >= h, "landscape source must stay landscape, got {w}x{h}");
    // Ratio drift ≤ 1 pixel (integer rounding).
    let src_ratio = 1000.0_f64 / 500.0;
    let thumb_ratio = w as f64 / h as f64;
    assert!(
        (src_ratio - thumb_ratio).abs() < 0.05,
        "aspect drift too large: src={src_ratio} thumb={thumb_ratio}"
    );

    // Portrait 500x1000 must stay portrait.
    let png_p = synth_rgb_png(500, 1000);
    let (_, w, h) = thumbnail(&png_p, 300, 300).expect("portrait thumb");
    assert!(h >= w, "portrait source must stay portrait, got {w}x{h}");
    let src_ratio_p = 500.0_f64 / 1000.0;
    let thumb_ratio_p = w as f64 / h as f64;
    assert!(
        (src_ratio_p - thumb_ratio_p).abs() < 0.05,
        "portrait aspect drift: src={src_ratio_p} thumb={thumb_ratio_p}"
    );

    // Square stays square.
    let png_s = synth_rgb_png(400, 400);
    let (_, w, h) = thumbnail(&png_s, 100, 100).expect("square thumb");
    assert_eq!(w, h, "square source must yield square thumb");
}
