//! Windows bitmap paste-back for stored images.
//!
//! Every decode is bounded by `max_decoded_image_mb`; synced or imported image
//! metadata cannot be trusted to describe the allocation it will require.

use std::io::{Cursor, Seek, SeekFrom, Write};

use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};

pub(super) enum CapturedImage {
    Image(Vec<u8>),
    TooLarge(u64),
    DecodedTooLarge,
    Invalid,
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

/// Verify a registered image representation before retaining its compressed
/// bytes. A tiny image can describe an unbounded decoded bitmap, so the
/// capture boundary enforces the same decoded-memory limit as paste-back.
pub(super) fn checked_image(
    encoded: Vec<u8>,
    format: ImageFormat,
    decoded_memory_mb: u32,
) -> CapturedImage {
    match decode(Cursor::new(&encoded), format, decoded_memory_mb) {
        Ok(_) => CapturedImage::Image(encoded),
        Err(ImageError::Limits(_)) => CapturedImage::DecodedTooLarge,
        Err(_) => CapturedImage::Invalid,
    }
}

/// Convert a `CF_DIB` payload into the PNG representation CopyPaste stores.
///
/// `BmpDecoder::new_without_file_header` is image-rs's native DIB path. The
/// clipboard API exposes its allocation size before `get_vec` copies it; this
/// helper applies the decoded-memory cap before image-rs allocates pixels.
pub(super) fn png_from_dib(dib: &[u8], decoded_memory_mb: u32, encoded_cap: u64) -> CapturedImage {
    match decode_dib(dib, decoded_memory_mb) {
        Ok(image) => encode_png_bounded(image, encoded_cap),
        Err(ImageError::Limits(_)) => CapturedImage::DecodedTooLarge,
        Err(_) => CapturedImage::Invalid,
    }
}

fn decode_dib(dib: &[u8], decoded_memory_mb: u32) -> image::ImageResult<DynamicImage> {
    let mut limits = limits(decoded_memory_mb);
    let mut decoder = image::codecs::bmp::BmpDecoder::new_without_file_header(Cursor::new(dib))?;
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    DynamicImage::from_decoder(decoder)
}

fn guess_and_decode(bytes: &[u8], decoded_memory_mb: u32) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits(decoded_memory_mb));
    reader.decode().ok()
}

fn decode(
    bytes: Cursor<impl AsRef<[u8]>>,
    format: ImageFormat,
    decoded_memory_mb: u32,
) -> image::ImageResult<DynamicImage> {
    let mut reader = ImageReader::with_format(bytes, format);
    reader.limits(limits(decoded_memory_mb));
    reader.decode()
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

fn encode_png_bounded(image: DynamicImage, cap: u64) -> CapturedImage {
    let mut encoded = BoundedVec::new(cap);
    match image.write_to(&mut encoded, ImageFormat::Png) {
        Ok(()) => CapturedImage::Image(encoded.into_inner()),
        Err(_) => encoded
            .exceeded()
            .map(CapturedImage::TooLarge)
            .unwrap_or(CapturedImage::Invalid),
    }
}

struct BoundedVec {
    bytes: Vec<u8>,
    cap: u64,
    position: u64,
    exceeded: Option<u64>,
}

impl BoundedVec {
    fn new(cap: u64) -> Self {
        Self {
            bytes: Vec::new(),
            cap,
            position: 0,
            exceeded: None,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    fn exceeded(&self) -> Option<u64> {
        self.exceeded
    }
}

impl Write for BoundedVec {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.position.saturating_add(bytes.len() as u64);
        if end > self.cap {
            self.exceeded = Some(end);
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "encoded image exceeds the capture limit",
            ));
        }
        let end = usize::try_from(end).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::WriteZero, "encoded image is too large")
        })?;
        let start = self.position as usize;
        if end > self.bytes.len() {
            self.bytes.resize(end, 0);
        }
        self.bytes[start..end].copy_from_slice(bytes);
        self.position = end as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for BoundedVec {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let base = match position {
            SeekFrom::Start(position) => 0i128.checked_add(position as i128),
            SeekFrom::Current(offset) => (self.position as i128).checked_add(offset as i128),
            SeekFrom::End(offset) => (self.bytes.len() as i128).checked_add(offset as i128),
        }
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid seek"))?;
        let position = u64::try_from(base)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid seek"))?;
        if position > self.cap {
            self.exceeded = Some(position);
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "encoded image exceeds the capture limit",
            ));
        }
        self.position = position;
        Ok(position)
    }
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

    #[test]
    fn a_native_dib_becomes_a_png() {
        let bitmap = to_bitmap(&png(8, 8), 50).unwrap();
        let CapturedImage::Image(png) = png_from_dib(&bitmap[14..], 50, 4 * 1024 * 1024) else {
            panic!("a DIB made by the maintained bitmap codec must decode");
        };
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn a_native_dib_over_the_decoded_budget_is_refused() {
        let bitmap = to_bitmap(&png(1024, 1024), 50).unwrap();
        assert!(matches!(
            png_from_dib(&bitmap[14..], 1, 4 * 1024 * 1024),
            CapturedImage::DecodedTooLarge
        ));
    }

    #[test]
    fn a_registered_png_over_the_decoded_budget_is_refused() {
        assert!(matches!(
            checked_image(png(1024, 1024), ImageFormat::Png, 1),
            CapturedImage::DecodedTooLarge
        ));
    }

    #[test]
    fn a_truncated_dib_does_not_reach_the_decoder() {
        assert!(matches!(
            png_from_dib(&[40, 0, 0, 0], 50, 4 * 1024 * 1024),
            CapturedImage::Invalid
        ));
    }
}
