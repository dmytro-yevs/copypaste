//! Shell icon extraction through WinSafe-owned handles and guards.

use std::path::Path;

use winsafe::guard::{DeleteObjectGuard, DestroyIconShfiGuard};
use winsafe::{co, prelude::*, BITMAPINFO, HDC, HICON};

use crate::AppIcon;

use super::{gdi, registry};

pub(super) fn resolve(image_name: &str) -> Option<AppIcon> {
    let path = registry::executable(image_name)?;
    let icon = shell_icon(&path)?;
    let pixels = icon_pixels(&icon.hIcon)?;
    AppIcon::from_png(pixels.png, pixels.width, pixels.height)
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
    let color = unsafe { DeleteObjectGuard::new(info.hbmColor) };
    let _mask = unsafe { DeleteObjectGuard::new(info.hbmMask) };
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

    #[test]
    #[ignore = "drives the real Windows shell"]
    fn a_real_shell_icon_becomes_a_bounded_png() {
        let path = registry::executable("cmd.exe").expect("cmd.exe is in System32");
        let icon = shell_icon(&path).expect("the shell has an icon for cmd.exe");
        let pixels = icon_pixels(&icon.hIcon).expect("the icon converts to PNG");
        assert!(pixels.width > 0 && pixels.height > 0);
        assert!(pixels.png.starts_with(b"\x89PNG"));
        assert!(pixels.png.len() <= crate::MAX_ICON_BYTES);
        assert!(pixels.width <= crate::MAX_ICON_EDGE);
        assert!(pixels.height <= crate::MAX_ICON_EDGE);
    }
}
