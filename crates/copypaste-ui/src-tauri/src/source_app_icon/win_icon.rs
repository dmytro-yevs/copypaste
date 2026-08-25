//! Shell icon extraction through WinSafe-owned handles and guards.

use std::path::Path;

use winsafe::guard::{DeleteObjectGuard, DestroyIconShfiGuard};
use winsafe::{co, prelude::*, BITMAPINFO, HDC, HICON};

use super::{gdi, registry};
use crate::model::UiSourceAppIcon;

pub(super) fn resolve(image_name: &str) -> Option<UiSourceAppIcon> {
    let path = registry::executable(image_name)?;
    let icon = shell_icon(&path)?;
    let pixels = icon_pixels(&icon.hIcon)?;
    Some(UiSourceAppIcon::from_png(
        pixels.png,
        pixels.width,
        pixels.height,
    ))
}

fn shell_icon(path: &Path) -> Option<DestroyIconShfiGuard> {
    let (_, info) = winsafe::SHGetFileInfo(
        path.to_str()?,
        co::FILE_ATTRIBUTE::NORMAL,
        co::SHGFI::ICON | co::SHGFI::LARGEICON,
    )
    .ok()?;
    info.hIcon.as_opt()?;
    Some(info)
}

fn icon_pixels(icon: &HICON) -> Option<gdi::IconPng> {
    let info = icon.GetIconInfo().ok()?;
    // SAFETY: GetIconInfo transfers both bitmap handles to its caller; these
    // guards delete each exactly once on every return path.
    let color = unsafe { DeleteObjectGuard::new(info.hbmColor) };
    let _mask = unsafe { DeleteObjectGuard::new(info.hbmMask) };
    // A monochrome icon carries no colour bitmap and a double-height mask.
    color.as_opt()?;

    let bitmap = color.GetObject().ok()?;
    if bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        return None;
    }
    let width = bitmap.bmWidth as u32;
    let height = bitmap.bmHeight as u32;

    let dc = HDC::NULL.CreateCompatibleDC().ok()?;
    let mut description = top_down_bgra(width, height);
    gdi::png_from_dib(width, height, |pixels| unsafe {
        dc.GetDIBits(
            &color,
            0,
            height,
            Some(pixels),
            &mut description,
            co::DIB::RGB_COLORS,
        )
        .unwrap_or(0)
    })
}

/// A negative height is what asks GDI for top-down rows, which is the order
/// `image` expects; a positive one would deliver the icon upside down.
fn top_down_bgra(width: u32, height: u32) -> BITMAPINFO {
    let mut description = BITMAPINFO::default();
    description.bmiHeader.biWidth = width as i32;
    description.bmiHeader.biHeight = -(height as i32);
    description.bmiHeader.biPlanes = 1;
    description.bmiHeader.biBitCount = 32;
    description.bmiHeader.biCompression = co::BI::RGB;
    description
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_system_executable_is_found_without_an_app_paths_entry() {
        let path = registry::executable("cmd.exe").expect("cmd.exe is in System32");
        assert!(path.is_file());
        assert!(path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe")));
    }

    #[test]
    fn an_unknown_image_name_resolves_to_no_path_and_no_icon() {
        assert!(registry::executable("copypaste-no-such-app.exe").is_none());
        assert!(resolve("copypaste-no-such-app.exe").is_none());
    }

    /// `#[ignore]` for the reason every real-shell test here carries one, and
    /// it is not hypothetical: under concurrent load `SHGetFileInfoW` returns
    /// *success* with a null `hIcon`, so a parallel `cargo test` fails on the
    /// shell's own transient answer. Everything that is ours — the App Paths
    /// search, the value decoding, the DIB bounds and the PNG cap — is covered
    /// deterministically in `registry` and `gdi`.
    #[test]
    #[ignore = "drives the real Windows shell"]
    fn a_real_shell_icon_becomes_a_bounded_png() {
        let path = registry::executable("cmd.exe").expect("cmd.exe is in System32");
        let icon = shell_icon(&path).expect("the shell has an icon for cmd.exe");
        let pixels = icon_pixels(&icon.hIcon).expect("the icon converts to PNG");
        assert!(pixels.width > 0 && pixels.height > 0);
        assert!(pixels.png.starts_with(b"\x89PNG"));
        assert!(pixels.png.len() <= super::super::MAX_ICON_BYTES);
    }
}
