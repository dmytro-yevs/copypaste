//! Bitmap on the clipboard, PNG everywhere else.
//!
//! Windows puts screenshots on the clipboard as an uncompressed bitmap; the
//! rest of CopyPaste stores PNG, which is what the content-type vocabulary
//! names, what the preview decoder reads and what a peer can paste. Storing the
//! bitmap as `image/bmp` instead would put a type outside that vocabulary into
//! the database and onto every peer. The cost, stated rather than hidden: a
//! decode and a re-encode per image copied, and the `bmp` feature of `image` on
//! Windows only.
//!
//! Every decode is bounded by `max_decoded_image_mb`: any application can put a
//! bitmap claiming 40000×40000 pixels on the clipboard.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader, Limits};

/// The clipboard's bitmap, as PNG.
pub(super) fn bitmap_to_png(bitmap: &[u8], decoded_memory_mb: u32) -> Option<Vec<u8>> {
    encode(
        decode(bitmap, ImageFormat::Bmp, decoded_memory_mb)?,
        ImageFormat::Png,
    )
}

/// A stored image, as the bitmap `SetClipboardData` wants.
///
/// Flattened to 8-bit RGB: `CreateDIBitmap` is fed a `BI_RGB` header, and an
/// alpha channel that the clipboard cannot carry is better dropped here than
/// reinterpreted as colour by whichever application pastes it.
pub(super) fn to_bitmap(encoded: &[u8], decoded_memory_mb: u32) -> Option<Vec<u8>> {
    let image = guess_and_decode(encoded, decoded_memory_mb)?;
    encode(DynamicImage::ImageRgb8(image.into_rgb8()), ImageFormat::Bmp)
}

fn decode(bytes: &[u8], format: ImageFormat, decoded_memory_mb: u32) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    reader.limits(limits(decoded_memory_mb));
    reader.decode().ok()
}

fn guess_and_decode(bytes: &[u8], decoded_memory_mb: u32) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits(decoded_memory_mb));
    reader.decode().ok()
}

fn limits(decoded_memory_mb: u32) -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(u64::from(decoded_memory_mb).saturating_mul(1024 * 1024));
    limits
}

fn encode(image: DynamicImage, format: ImageFormat) -> Option<Vec<u8>> {
    let mut encoded = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut encoded), format)
        .ok()?;
    Some(encoded)
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::*;

    fn bitmap(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgb([0x24u8, 0x65, 0xa8]));
        encode(DynamicImage::ImageRgb8(image), ImageFormat::Bmp).unwrap()
    }

    #[test]
    fn a_clipboard_bitmap_becomes_a_png_of_the_same_size() {
        let png = bitmap_to_png(&bitmap(64, 32), 50).expect("the bitmap must transcode");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = guess_and_decode(&png, 50).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 32));
    }

    #[test]
    fn a_stored_png_becomes_a_bitmap_the_clipboard_can_take() {
        let png = bitmap_to_png(&bitmap(8, 8), 50).unwrap();
        let bmp = to_bitmap(&png, 50).expect("the png must transcode");
        assert_eq!(&bmp[..2], b"BM");
    }

    /// The budget is the whole reason this goes through `ImageReader` rather
    /// than `image::load_from_memory`: any application can put a bitmap on the
    /// clipboard claiming dimensions that would allocate gigabytes.
    #[test]
    fn an_image_beyond_the_decoded_budget_is_refused_rather_than_allocated() {
        assert!(bitmap_to_png(&bitmap(1024, 1024), 1).is_none());
        assert!(bitmap_to_png(&bitmap(1024, 1024), 50).is_some());
    }

    #[test]
    fn a_payload_that_is_not_an_image_is_refused() {
        assert!(bitmap_to_png(b"not a bitmap", 50).is_none());
        assert!(to_bitmap(b"not an image", 50).is_none());
    }
}
