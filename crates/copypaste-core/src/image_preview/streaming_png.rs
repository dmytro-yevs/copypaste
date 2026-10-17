//! Feed image-rs' existing thumbnail sampler from a bounded PNG row window.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Cursor;
use std::marker::PhantomData;

use image::{ColorType, GenericImageView, ImageEncoder, Pixel};

use super::{decode_limits, ImagePreviewError, ImageThumbnail};

const MAX_ROW_WINDOW_BYTES: usize = 1024 * 1024;

pub(super) fn thumbnail(
    source: &[u8],
    budget: u64,
    max_width: u32,
    max_height: u32,
) -> Result<Option<ImageThumbnail>, ImagePreviewError> {
    let mut decoder = png::Decoder::new(Cursor::new(source));
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(budget).unwrap_or(usize::MAX),
    });
    decoder.set_transformations(png::Transformations::EXPAND);
    let reader = decoder.read_info().map_err(|_| ImagePreviewError::Decode)?;
    let info = reader.info();
    let (color, depth) = reader.output_color_type();
    // Adam7/APNG need different access patterns. Keep their existing bounded
    // decoder, and never strip 16-bit channels to obtain the streaming path.
    if info.interlaced || info.animation_control.is_some() || depth != png::BitDepth::Eight {
        return Ok(None);
    }
    let (width, height) = (info.width, info.height);
    let channels = color.samples() as u8;
    let source_bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(u64::from(channels));
    // Preserve the existing decoded-size acceptance policy even though the
    // streaming implementation does not allocate that full framebuffer.
    decode_limits(
        width,
        height,
        source_bytes,
        channels,
        max_width,
        max_height,
        budget,
    )?;
    let ratio = (f64::from(max_width.min(width)) / f64::from(width))
        .min(f64::from(max_height.min(height)) / f64::from(height));
    let target = (
        (f64::from(width) * ratio).round().max(1.0) as u32,
        (f64::from(height) * ratio).round().max(1.0) as u32,
    );
    // The sampler traverses output rows, then columns, revisiting source rows
    // within one output-row band. Include rounding/neighbor headroom, and fall
    // back for shapes whose required window would exceed the scratch bound.
    let y_ratio = height as f32 / target.1 as f32;
    let capacity = (0..target.1)
        .map(|y| {
            let lower = y as f32 * y_ratio;
            let bottom = (lower.ceil() as u32).min(height - 1);
            let top = ((lower + y_ratio).ceil() as u32).clamp(bottom, height);
            (top - bottom) as usize
        })
        .max()
        .unwrap_or(0)
        .saturating_add(2);
    let row_bytes = reader
        .output_line_size(width)
        .ok_or(ImagePreviewError::TooLarge)?;
    if capacity.saturating_mul(row_bytes) > MAX_ROW_WINDOW_BYTES {
        return Ok(None);
    }
    let rows = Rows {
        reader,
        rows: VecDeque::new(),
        first: 0,
        capacity,
        row_bytes,
        error: None,
    };
    match color {
        png::ColorType::Grayscale => {
            encode::<image::Luma<u8>>(rows, (width, height), target, ColorType::L8)
        }
        png::ColorType::GrayscaleAlpha => {
            encode::<image::LumaA<u8>>(rows, (width, height), target, ColorType::La8)
        }
        png::ColorType::Rgb => {
            encode::<image::Rgb<u8>>(rows, (width, height), target, ColorType::Rgb8)
        }
        png::ColorType::Rgba => {
            encode::<image::Rgba<u8>>(rows, (width, height), target, ColorType::Rgba8)
        }
        png::ColorType::Indexed => Ok(None),
    }
}

struct Rows<'a> {
    reader: png::Reader<Cursor<&'a [u8]>>,
    rows: VecDeque<Vec<u8>>,
    first: u32,
    capacity: usize,
    row_bytes: usize,
    error: Option<ImagePreviewError>,
}

struct RowImage<'a, P> {
    dimensions: (u32, u32),
    state: RefCell<Rows<'a>>,
    pixel: PhantomData<P>,
}

impl<P: Pixel<Subpixel = u8>> GenericImageView for RowImage<'_, P> {
    type Pixel = P;

    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn get_pixel(&self, x: u32, y: u32) -> P {
        let zero = || *P::from_slice(&[0; 4][..usize::from(P::CHANNEL_COUNT)]);
        let mut state = self.state.borrow_mut();
        if state.error.is_some() {
            return zero();
        }
        while y >= state.first + state.rows.len() as u32 {
            let mut row = if state.rows.len() == state.capacity {
                state.first += 1;
                state.rows.pop_front().expect("nonempty bounded row window")
            } else {
                vec![0; state.row_bytes]
            };
            match state.reader.read_row(&mut row) {
                Ok(Some(_)) => state.rows.push_back(row),
                _ => {
                    state.error = Some(ImagePreviewError::Decode);
                    return zero();
                }
            }
        }
        let Some(index) = y.checked_sub(state.first) else {
            // Do not return a partial preview if a future sampler version
            // changes its traversal beyond the supported row window.
            state.error = Some(ImagePreviewError::Decode);
            return zero();
        };
        let channels = usize::from(P::CHANNEL_COUNT);
        let offset = x as usize * channels;
        *P::from_slice(&state.rows[index as usize][offset..offset + channels])
    }
}

fn encode<P: Pixel<Subpixel = u8> + 'static>(
    rows: Rows<'_>,
    dimensions: (u32, u32),
    target: (u32, u32),
    color: ColorType,
) -> Result<Option<ImageThumbnail>, ImagePreviewError> {
    let view = RowImage::<P> {
        dimensions,
        state: RefCell::new(rows),
        pixel: PhantomData,
    };
    let image = image::imageops::thumbnail(&view, target.0, target.1);
    let mut state = view.state.into_inner();
    if let Some(error) = state.error.take() {
        return Err(error);
    }
    // Validate remaining compressed data/CRC instead of accepting a partially
    // decoded image just because the requested sample rows were available.
    while state
        .reader
        .next_row()
        .map_err(|_| ImagePreviewError::Decode)?
        .is_some()
    {}
    state
        .reader
        .finish()
        .map_err(|_| ImagePreviewError::Decode)?;
    drop(state);
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(image.as_raw(), target.0, target.1, color.into())
        .map_err(|_| ImagePreviewError::Encode)?;
    Ok(Some(ImageThumbnail {
        png,
        width: target.0,
        height: target.1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};

    fn encoded(image: &DynamicImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn compare(source: &[u8], bounds: (u32, u32)) {
        let original = image::load_from_memory(source).unwrap();
        let expected = original.thumbnail(
            original.width().min(bounds.0),
            original.height().min(bounds.1),
        );
        let actual = crate::thumbnail_png(source, 50, None, Some(bounds)).unwrap();
        assert_eq!(
            (actual.width, actual.height),
            (expected.width(), expected.height())
        );
        assert_eq!(
            actual.png,
            encoded(&expected),
            "thumbnail bytes changed at {:?}",
            bounds
        );
    }

    #[test]
    fn streaming_matches_existing_sampler_and_encoder_byte_for_byte() {
        for (width, height) in [
            (17, 13),
            (257, 113),
            (3, 127),
            (127, 3),
            (1, 31),
            (31, 1),
            (64, 64),
        ] {
            let rgba = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
                Rgba([
                    (x * 137 + y * 29) as u8,
                    (x * 7 + y * 31) as u8,
                    (x ^ y) as u8,
                    (x * 13 + y * 17) as u8,
                ])
            }));
            for source in [
                rgba.clone(),
                DynamicImage::ImageRgb8(rgba.to_rgb8()),
                DynamicImage::ImageLuma8(rgba.to_luma8()),
                DynamicImage::ImageLumaA8(rgba.to_luma_alpha8()),
            ] {
                let source = encoded(&source);
                for bounds in [(1, 1), (7, 5), (63, 31), (1400, 320), (1024, 1024)] {
                    assert!(thumbnail(&source, 50 * 1024 * 1024, bounds.0, bounds.1)
                        .unwrap()
                        .is_some());
                    compare(&source, bounds);
                }
            }
        }
    }

    #[test]
    fn palette_transparency_is_preserved() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![255, 0, 0, 0, 255, 0, 0, 0, 255]);
            encoder.set_trns(vec![0, 127, 255]);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[0, 1, 2, 0])
                .unwrap();
        }
        compare(&bytes, (1, 1));
    }

    #[test]
    fn sixteen_bit_and_animation_keep_the_existing_decoder() {
        let sixteen = encoded(&DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            4,
            4,
            Rgba([1234u16, 5678, 9012, 65535]),
        )));
        assert!(thumbnail(&sixteen, 50 * 1024 * 1024, 2, 2)
            .unwrap()
            .is_none());
        compare(&sixteen, (2, 2));
        let mut animated = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut animated, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(2, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255; 16]).unwrap();
            writer.write_image_data(&[128; 16]).unwrap();
            writer.finish().unwrap();
        }
        assert!(thumbnail(&animated, 50 * 1024 * 1024, 1, 1)
            .unwrap()
            .is_none());
        compare(&animated, (1, 1));
    }

    #[test]
    fn corrupted_rows_fail_without_poisoning_later_previews() {
        let good = encoded(&DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            64,
            64,
            Rgba([10, 20, 30, 255]),
        )));
        let mut bad = good.clone();
        let middle = bad.len() / 2;
        bad[middle] ^= 0xff;
        assert!(crate::thumbnail_png(&bad, 50, Some(32), None).is_err());
        compare(&good, (32, 32));
    }
}
