//! Safe, bounded thumbnails for binary clipboard images.

use std::io::Cursor;

use image::{ImageFormat, ImageReader, Limits};
use thiserror::Error;

/// A preview is deliberately much smaller than its source. It is displayed in
/// a history row, never used to restore an image to the system clipboard.
pub const MAX_THUMBNAIL_EDGE: u32 = 384;

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
) -> Result<ImageThumbnail, ImagePreviewError> {
    let budget = u64::from(decoded_memory_mb).saturating_mul(1024 * 1024);
    let thumbnail_bytes = u64::from(MAX_THUMBNAIL_EDGE)
        .saturating_mul(u64::from(MAX_THUMBNAIL_EDGE))
        .saturating_mul(4);
    let source_budget = budget
        .checked_sub(thumbnail_bytes)
        .ok_or(ImagePreviewError::TooLarge)?;

    let mut limits = Limits::default();
    limits.max_alloc = Some(source_budget);
    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| ImagePreviewError::Decode)?;
    reader.limits(limits);
    let image = reader.decode().map_err(|error| {
        if matches!(error, image::ImageError::Limits(_)) {
            ImagePreviewError::TooLarge
        } else {
            ImagePreviewError::Decode
        }
    })?;
    let thumbnail = image.thumbnail(
        image.width().min(MAX_THUMBNAIL_EDGE),
        image.height().min(MAX_THUMBNAIL_EDGE),
    );
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
        let thumbnail = thumbnail_png(&png(1200, 600), 50).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (384, 192));
        assert_eq!(&thumbnail.png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn refuses_a_decode_over_the_budget() {
        assert!(matches!(
            thumbnail_png(&png(1200, 1200), 1),
            Err(ImagePreviewError::TooLarge)
        ));
    }

    #[test]
    fn refuses_non_images() {
        assert!(matches!(
            thumbnail_png(b"not an image", 50),
            Err(ImagePreviewError::Decode)
        ));
    }
}
