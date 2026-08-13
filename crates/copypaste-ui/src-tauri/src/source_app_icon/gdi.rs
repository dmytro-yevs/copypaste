//! Icon pixels, bounded and converted to PNG before they reach the WebView.

use std::io::Cursor;

use image::{ImageBuffer, ImageFormat, RgbaImage};

use super::{MAX_ICON_BYTES, MAX_ICON_EDGE};

pub(super) struct IconPng {
    pub(super) png: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Convert a 32-bit top-down DIB to a bounded PNG.
///
/// `copy_rows` fills the buffer and returns what `GetDIBits` returns: the
/// number of scan lines copied. Anything between `1` and `height - 1` left the
/// rest of the buffer as the zeroed allocation it started as, so encoding it
/// would publish a half-blank image as the application's own icon — the reason
/// a plain "nonzero means success" test is wrong here.
pub(super) fn png_from_dib(
    width: u32,
    height: u32,
    copy_rows: impl FnOnce(&mut [u8]) -> i32,
) -> Option<IconPng> {
    if width == 0 || height == 0 || width > MAX_ICON_EDGE || height > MAX_ICON_EDGE {
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
    // A cap here rather than at the WebView boundary: the icon rides an IPC
    // response beside every history row.
    (png.len() <= MAX_ICON_BYTES).then_some(IconPng { png, width, height })
}

/// GDI hands back BGRA. An icon whose alpha is zero everywhere is an older
/// 32-bit DIB that never set the channel, not an invisible icon.
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

    use super::*;

    /// One opaque blue pixel as GDI would hand it over: B, G, R, A.
    fn blue(pixels: &mut [u8]) {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0xF0, 0x20, 0x10, 0xFF]);
        }
    }

    fn decoded(icon: &IconPng) -> RgbaImage {
        assert!(icon.png.starts_with(b"\x89PNG"));
        image::load_from_memory_with_format(&icon.png, ImageFormat::Png)
            .expect("the icon is a PNG")
            .to_rgba8()
    }

    #[test]
    fn a_full_copy_becomes_a_png_with_the_channels_in_order() {
        let icon = png_from_dib(4, 4, |pixels| {
            blue(pixels);
            4
        })
        .expect("a complete copy encodes");
        let image = decoded(&icon);
        assert_eq!(image.dimensions(), (4, 4));
        assert_eq!(
            image.get_pixel(0, 0).0,
            [0x10, 0x20, 0xF0, 0xFF],
            "BGRA must reach the WebView as RGBA"
        );
    }

    /// The B4 defect: `GetDIBits` returns the number of scan lines it copied,
    /// so a short return means the tail of the buffer is still zeroed.
    #[test]
    fn a_partial_copy_is_refused() {
        for copied in [1, 7] {
            assert!(
                png_from_dib(8, 8, |pixels| {
                    blue(pixels);
                    copied
                })
                .is_none(),
                "{copied} of 8 rows was accepted as a whole icon"
            );
        }
    }

    #[test]
    fn a_failed_or_impossible_copy_is_refused() {
        assert!(png_from_dib(8, 8, |_| 0).is_none(), "GetDIBits failure");
        assert!(png_from_dib(8, 8, |_| -1).is_none(), "a negative return");
        assert!(
            png_from_dib(8, 8, |_| 9).is_none(),
            "more rows than the bitmap has"
        );
    }

    #[test]
    fn a_buffer_is_sized_for_every_row_before_the_copy_runs() {
        let seen = Cell::new(0);
        let icon = png_from_dib(3, 5, |pixels| {
            seen.set(pixels.len());
            blue(pixels);
            5
        });
        assert!(icon.is_some());
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
        assert_eq!(decoded(&icon).get_pixel(0, 0).0, [0x33, 0x22, 0x11, 0xFF]);
    }

    #[test]
    fn a_transparent_corner_survives_beside_opaque_pixels() {
        let icon = png_from_dib(2, 1, |pixels| {
            pixels[..4].copy_from_slice(&[0x10, 0x20, 0x30, 0x00]);
            pixels[4..].copy_from_slice(&[0x10, 0x20, 0x30, 0xFF]);
            1
        })
        .expect("encodes");
        let image = decoded(&icon);
        assert_eq!(image.get_pixel(0, 0).0[3], 0x00, "a real alpha is kept");
        assert_eq!(image.get_pixel(1, 0).0[3], 0xFF);
    }

    #[test]
    fn the_edge_bound_is_inclusive_and_a_larger_bitmap_is_never_copied() {
        let called = Cell::new(false);
        assert!(
            png_from_dib(MAX_ICON_EDGE + 1, 8, |_| {
                called.set(true);
                8
            })
            .is_none(),
            "an oversize bitmap was accepted"
        );
        assert!(
            !called.get(),
            "an oversize bitmap must be refused before any pixel is read"
        );
        assert!(png_from_dib(8, MAX_ICON_EDGE + 1, |_| 0).is_none());
        assert!(png_from_dib(0, 8, |_| 0).is_none());
        assert!(png_from_dib(8, 0, |_| 0).is_none());
        assert!(
            png_from_dib(MAX_ICON_EDGE, 1, |pixels| {
                blue(pixels);
                1
            })
            .is_some(),
            "the bound itself must be accepted"
        );
    }

    /// The byte cap is what the WebView is protected by, and an icon at the
    /// edge bound can still exceed it: incompressible pixels at 512x512 are a
    /// megabyte of PNG.
    #[test]
    fn a_png_over_the_byte_cap_is_refused() {
        let icon = png_from_dib(MAX_ICON_EDGE, MAX_ICON_EDGE, |pixels| {
            let mut state = 0x2545_F491_4F6C_DD1Du64;
            for byte in pixels.iter_mut() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = (state >> 24) as u8;
            }
            MAX_ICON_EDGE as i32
        });
        assert!(
            icon.is_none(),
            "an icon larger than {MAX_ICON_BYTES} bytes reached the WebView"
        );
    }
}
