//! Safe, bounded thumbnails for binary clipboard images.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use thiserror::Error;

/// A preview is deliberately much smaller than its source. It is displayed in
/// a history row, never used to restore an image to the system clipboard.
pub const DEFAULT_THUMBNAIL_EDGE: u32 = 384;
pub const MAX_THUMBNAIL_EDGE: u32 = 2_048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageThumbnail {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Error)]
pub enum ImagePreviewError {
    #[error("the image could not be decoded")]
    Decode,
    #[error("the image exceeds the decoded-memory limit")]
    TooLarge,
    #[error("the thumbnail could not be encoded")]
    Encode,
}

/// Decode one clipboard image within the user's memory budget and encode a
/// PNG thumbnail. No source image bytes leave this function.
pub fn thumbnail_png(
    source: &[u8],
    decoded_memory_mb: u32,
    max_edge: Option<u32>,
) -> Result<ImageThumbnail, ImagePreviewError> {
    let edge = max_edge
        .unwrap_or(DEFAULT_THUMBNAIL_EDGE)
        .clamp(1, MAX_THUMBNAIL_EDGE);
    let budget = u64::from(decoded_memory_mb).saturating_mul(1024 * 1024);
    let mut limits = Limits::default();
    limits.max_alloc = Some(budget);
    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| ImagePreviewError::Decode)?;
    reader.limits(limits.clone());
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| ImagePreviewError::Decode)?;
    let (source_width, source_height) = decoder.dimensions();
    if source_width == 0 || source_height == 0 {
        return Err(ImagePreviewError::Decode);
    }
    let longest = source_width.max(source_height);
    let target_width = u64::from(source_width)
        .saturating_mul(u64::from(edge))
        .div_ceil(u64::from(longest))
        .min(u64::from(source_width));
    let target_height = u64::from(source_height)
        .saturating_mul(u64::from(edge))
        .div_ceil(u64::from(longest))
        .min(u64::from(source_height));
    let thumbnail_bytes = target_width
        .saturating_mul(target_height)
        .saturating_mul(u64::from(decoder.color_type().bytes_per_pixel()));
    limits
        .reserve(thumbnail_bytes)
        .map_err(|_| ImagePreviewError::TooLarge)?;
    limits
        .reserve(decoder.total_bytes())
        .map_err(|_| ImagePreviewError::TooLarge)?;
    decoder
        .set_limits(limits)
        .map_err(|_| ImagePreviewError::TooLarge)?;
    let image = DynamicImage::from_decoder(decoder).map_err(|error| {
        if matches!(error, image::ImageError::Limits(_)) {
            ImagePreviewError::TooLarge
        } else {
            ImagePreviewError::Decode
        }
    })?;
    let thumbnail = image.thumbnail(image.width().min(edge), image.height().min(edge));
    let (width, height) = (thumbnail.width(), thumbnail.height());
    let mut png = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|_| ImagePreviewError::Encode)?;

    Ok(ImageThumbnail { png, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba([0x24u8, 0x65, 0xa8, 0xff]));
        let mut encoded = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .unwrap();
        encoded
    }

    #[test]
    fn creates_a_png_thumbnail() {
        let thumbnail = thumbnail_png(&png(1200, 600), 50, None).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (384, 192));
        assert_eq!(&thumbnail.png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn refuses_a_decode_over_the_budget() {
        assert!(matches!(
            thumbnail_png(&png(1200, 1200), 1, None),
            Err(ImagePreviewError::TooLarge)
        ));
    }

    #[test]
    fn refuses_non_images() {
        assert!(matches!(
            thumbnail_png(b"not an image", 50, None),
            Err(ImagePreviewError::Decode)
        ));
    }

    #[test]
    fn high_dpi_edge_does_not_upscale_small_images() {
        assert_eq!(
            thumbnail_png(&png(1600, 800), 50, Some(1024))
                .unwrap()
                .width,
            1024
        );
        assert_eq!(
            thumbnail_png(&png(120, 60), 50, Some(1024)).unwrap().width,
            120
        );
    }

    #[test]
    fn tiny_image_with_high_dpi_request_fits_a_small_budget() {
        let thumbnail = thumbnail_png(&png(2, 2), 1, Some(2048)).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (2, 2));
    }
}
