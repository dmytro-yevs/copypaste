//! The shell's icon for an executable, and the GDI handles that come with it.
//!
//! Every handle here is owned by a guard: the previous shape deleted each
//! bitmap on each of six early returns, which is one edit away from a leak in
//! the process that draws the history list.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::{mem, ptr};

use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

use super::{gdi, registry};
use crate::model::UiSourceAppIcon;

pub(super) fn resolve(image_name: &str) -> Option<UiSourceAppIcon> {
    let path = find_executable(image_name)?;
    let icon = shell_icon(&path)?;
    let pixels = icon_pixels(icon.0)?;
    Some(UiSourceAppIcon::from_png(
        pixels.png,
        pixels.width,
        pixels.height,
    ))
}

/// App Paths first, then System32: the tools Windows ships (`cmd.exe`,
/// `notepad.exe`) register no App Path at all.
pub(super) fn find_executable(image_name: &str) -> Option<PathBuf> {
    if let Some(path) = registry::app_path(image_name) {
        return Some(path);
    }
    let root = std::env::var_os("SystemRoot")?;
    let path = PathBuf::from(root).join("System32").join(image_name);
    path.is_file().then_some(path)
}

fn shell_icon(path: &Path) -> Option<Icon> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL terminated and `info` is sized for the call.
    let mut info: SHFILEINFOW = unsafe { mem::zeroed() };
    let answered = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &raw mut info,
            mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    (answered != 0 && !info.hIcon.is_null()).then_some(Icon(info.hIcon))
}

fn icon_pixels(icon: *mut c_void) -> Option<gdi::IconPng> {
    // SAFETY: `icon` is a live HICON owned by the caller's guard.
    let mut info: ICONINFO = unsafe { mem::zeroed() };
    if unsafe { GetIconInfo(icon, &raw mut info) } == 0 {
        return None;
    }
    let color = GdiObject(info.hbmColor);
    let _mask = GdiObject(info.hbmMask);
    // A monochrome icon carries no colour bitmap and a double-height mask.
    if color.0.is_null() {
        return None;
    }

    let mut bitmap: BITMAP = unsafe { mem::zeroed() };
    let described = unsafe {
        GetObjectW(
            color.0,
            mem::size_of::<BITMAP>() as i32,
            (&raw mut bitmap).cast(),
        )
    };
    if described == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        return None;
    }
    let width = bitmap.bmWidth as u32;
    let height = bitmap.bmHeight as u32;

    let dc = MemoryDc::create()?;
    let mut description = top_down_bgra(width, height);
    gdi::png_from_dib(width, height, |pixels| unsafe {
        GetDIBits(
            dc.0,
            color.0,
            0,
            height,
            pixels.as_mut_ptr().cast(),
            &raw mut description,
            DIB_RGB_COLORS,
        )
    })
}

/// A negative height is what asks GDI for top-down rows, which is the order
/// `image` expects; a positive one would deliver the icon upside down.
fn top_down_bgra(width: u32, height: u32) -> BITMAPINFO {
    // SAFETY: `BITMAPINFO` is a plain C struct with no invalid bit patterns.
    let mut description: BITMAPINFO = unsafe { mem::zeroed() };
    description.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
    description.bmiHeader.biWidth = width as i32;
    description.bmiHeader.biHeight = -(height as i32);
    description.bmiHeader.biPlanes = 1;
    description.bmiHeader.biBitCount = 32;
    description.bmiHeader.biCompression = BI_RGB;
    description
}

struct Icon(*mut c_void);

impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: the handle came from `SHGetFileInfoW` and is destroyed once.
        unsafe { DestroyIcon(self.0) };
    }
}

struct GdiObject(*mut c_void);

impl Drop for GdiObject {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // SAFETY: the handle came from `GetIconInfo`, which hands ownership of
        // both bitmaps to the caller, and is deleted once.
        unsafe { DeleteObject(self.0) };
    }
}

struct MemoryDc(*mut c_void);

impl MemoryDc {
    fn create() -> Option<Self> {
        // SAFETY: a null argument asks for a DC compatible with the screen.
        let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
        (!dc.is_null()).then_some(Self(dc))
    }
}

impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: the DC came from `CreateCompatibleDC` and is deleted once.
        unsafe { DeleteDC(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_system_executable_is_found_without_an_app_paths_entry() {
        let path = find_executable("cmd.exe").expect("cmd.exe is in System32");
        assert!(path.is_file());
        assert!(path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe")));
    }

    #[test]
    fn an_unknown_image_name_resolves_to_no_path_and_no_icon() {
        assert!(find_executable("copypaste-no-such-app.exe").is_none());
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
        let path = find_executable("cmd.exe").expect("cmd.exe is in System32");
        let icon = shell_icon(&path).expect("the shell has an icon for cmd.exe");
        let pixels = icon_pixels(icon.0).expect("the icon converts to PNG");
        assert!(pixels.width > 0 && pixels.height > 0);
        assert!(pixels.png.starts_with(b"\x89PNG"));
        assert!(pixels.png.len() <= super::super::MAX_ICON_BYTES);
    }
}
