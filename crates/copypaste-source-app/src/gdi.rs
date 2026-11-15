//! Icon pixels, bounded and converted to PNG before persistence.

use std::io::Cursor;

use base64::Engine as _;
use image::{ImageBuffer, ImageFormat, RgbaImage};

use crate::AppIcon;

const MAX_GDI_ICON_EDGE: u32 = 512;

pub(super) struct IconPng {
    pub(super) png: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn png_from_dib(
    width: u32,
    height: u32,
    copy_rows: impl FnOnce(&mut [u8]) -> i32,
) -> Option<IconPng> {
    if width == 0 || height == 0 || width > MAX_GDI_ICON_EDGE || height > MAX_GDI_ICON_EDGE {
        return None;
    }
    let bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let mut pixels = vec![0u8; bytes];
    if copy_rows(&mut pixels) != height as i32 {
        return None;
    }
    to_rgba(&mut pixels);
    let image: RgbaImage = ImageBuffer::from_raw(width, height, pixels)?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    let icon = AppIcon::from_png(png, width, height)?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(icon.png_base64)
        .ok()?;
    Some(IconPng {
        png,
        width: icon.width,
        height: icon.height,
    })
}

fn to_rgba(pixels: &mut [u8]) {
    let unset_alpha = pixels.chunks_exact(4).all(|pixel| pixel[3] == 0);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if unset_alpha {
            pixel[3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use base64::Engine as _;

    use super::*;

    #[test]
    fn a_full_copy_becomes_a_bounded_png() {
        let icon = png_from_dib(4, 4, |pixels| {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[0xF0, 0x20, 0x10, 0xFF]);
            }
            4
        })
        .expect("a complete copy encodes");
        assert!(icon.png.starts_with(b"\x89PNG"));
        let image = image::load_from_memory_with_format(&icon.png, ImageFormat::Png)
            .expect("the icon is a PNG")
            .to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, [0x10, 0x20, 0xF0, 0xFF]);
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(
                    AppIcon::from_png(icon.png.clone(), icon.width, icon.height)
                        .unwrap()
                        .png_base64
                )
                .unwrap()
                .len(),
            icon.png.len()
        );
    }

    #[test]
    fn a_partial_copy_is_refused() {
        for copied in [1, 7] {
            assert!(
                png_from_dib(8, 8, |_| copied).is_none(),
                "{copied} of 8 rows was accepted"
            );
        }
    }

    #[test]
    fn a_failed_or_impossible_copy_is_refused() {
        assert!(png_from_dib(8, 8, |_| 0).is_none());
        assert!(png_from_dib(8, 8, |_| -1).is_none());
        assert!(png_from_dib(8, 8, |_| 9).is_none());
    }

    #[test]
    fn a_buffer_is_sized_for_every_row_before_the_copy_runs() {
        let seen = Cell::new(0);
        assert!(png_from_dib(3, 5, |pixels| {
            seen.set(pixels.len());
            5
        })
        .is_some());
        assert_eq!(seen.get(), 3 * 5 * 4);
    }

    #[test]
    fn an_icon_with_no_alpha_channel_set_is_opaque() {
        let icon = png_from_dib(2, 2, |pixels| {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[0x11, 0x22, 0x33, 0x00]);
            }
            2
        })
        .expect("encodes");
        let image = image::load_from_memory_with_format(&icon.png, ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, [0x33, 0x22, 0x11, 0xFF]);
    }

    #[test]
    fn a_transparent_corner_survives_beside_opaque_pixels() {
        let icon = png_from_dib(2, 1, |pixels| {
            pixels[..4].copy_from_slice(&[0x10, 0x20, 0x30, 0x00]);
            pixels[4..].copy_from_slice(&[0x10, 0x20, 0x30, 0xFF]);
            1
        })
        .expect("encodes");
        let image = image::load_from_memory_with_format(&icon.png, ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0[3], 0x00);
        assert_eq!(image.get_pixel(1, 0).0[3], 0xFF);
    }

    #[test]
    fn native_edge_bound_is_inclusive_and_larger_bitmaps_are_not_read() {
        let called = Cell::new(false);
        assert!(png_from_dib(MAX_GDI_ICON_EDGE + 1, 8, |_| {
            called.set(true);
            8
        })
        .is_none());
        assert!(!called.get());
        assert!(png_from_dib(8, MAX_GDI_ICON_EDGE + 1, |_| 0).is_none());
        assert!(png_from_dib(0, 8, |_| 0).is_none());
        assert!(png_from_dib(8, 0, |_| 0).is_none());
        assert!(png_from_dib(MAX_GDI_ICON_EDGE, 1, |pixels| {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[0xF0, 0x20, 0x10, 0xFF]);
            }
            1
        })
        .is_some());
    }

    #[test]
    fn incompressible_icons_over_the_normalized_byte_cap_are_refused() {
        let icon = png_from_dib(MAX_GDI_ICON_EDGE, MAX_GDI_ICON_EDGE, |pixels| {
            let mut state = 0x2545_F491_4F6C_DD1Du64;
            for byte in pixels.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state >> 24) as u8;
            }
            MAX_GDI_ICON_EDGE as i32
        });
        assert!(icon.is_none());
    }
}
