//! Windows bitmap paste-back for stored images.
//!
//! Every decode is bounded by `max_decoded_image_mb`; synced or imported image
//! metadata cannot be trusted to describe the allocation it will require.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader, Limits};

/// A stored image, as the bitmap `SetClipboardData` wants.
///
/// Flattened to 8-bit RGB: `CreateDIBitmap` is fed a `BI_RGB` header, and an
/// alpha channel that the clipboard cannot carry is better dropped here than
/// reinterpreted as colour by whichever application pastes it.
pub(super) fn to_bitmap(encoded: &[u8], decoded_memory_mb: u32) -> Option<Vec<u8>> {
    let image = guess_and_decode(encoded, decoded_memory_mb)?;
    encode(DynamicImage::ImageRgb8(image.into_rgb8()), ImageFormat::Bmp)
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

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgb([0x24u8, 0x65, 0xa8]));
        encode(DynamicImage::ImageRgb8(image), ImageFormat::Png).unwrap()
    }

    #[test]
    fn a_stored_png_becomes_a_bitmap_the_clipboard_can_take() {
        let bmp = to_bitmap(&png(8, 8), 50).expect("the png must transcode");
        assert_eq!(&bmp[..2], b"BM");
    }

    /// The budget is the whole reason this goes through `ImageReader` rather
    /// than `image::load_from_memory`: any application can put a bitmap on the
    /// clipboard claiming dimensions that would allocate gigabytes.
    #[test]
    fn an_image_beyond_the_decoded_budget_is_refused_rather_than_allocated() {
        let image = png(1024, 1024);
        assert!(to_bitmap(&image, 1).is_none());
        assert!(to_bitmap(&image, 50).is_some());
    }

    #[test]
    fn a_payload_that_is_not_an_image_is_refused() {
        assert!(to_bitmap(b"not an image", 50).is_none());
    }
}
