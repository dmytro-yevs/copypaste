//! Native source-application icons, converted before they reach the WebView.

#![allow(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::model::UiSourceAppIcon;

const MAX_CACHE_ENTRIES: usize = 64;
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

struct CacheEntry {
    icon: UiSourceAppIcon,
    resolved_at: Instant,
}

#[derive(Default)]
pub struct SourceAppIconCache {
    entries: Mutex<VecDeque<(String, CacheEntry)>>,
}

impl SourceAppIconCache {
    pub fn resolve_desktop(&self, bundle_id: &str) -> Option<UiSourceAppIcon> {
        self.resolve_with(bundle_id, resolve_desktop)
    }

    pub fn resolve_with(
        &self,
        bundle_id: &str,
        resolver: impl FnOnce(&str) -> Option<UiSourceAppIcon>,
    ) -> Option<UiSourceAppIcon> {
        if !valid_package_id(bundle_id) {
            return None;
        }
        if let Some(icon) = self.cached(bundle_id) {
            return Some(icon);
        }
        let icon = resolver(bundle_id)?;
        self.insert(bundle_id.to_owned(), icon.clone());
        Some(icon)
    }

    fn cached(&self, bundle_id: &str) -> Option<UiSourceAppIcon> {
        let mut entries = self.entries.lock().expect("source icon cache");
        let index = entries.iter().position(|(key, _)| key == bundle_id)?;
        let entry = entries.remove(index).expect("entry index is valid");
        if entry.1.resolved_at.elapsed() >= CACHE_TTL {
            return None;
        }
        let icon = entry.1.icon.clone();
        entries.push_front(entry);
        Some(icon)
    }

    fn insert(&self, bundle_id: String, icon: UiSourceAppIcon) {
        let mut entries = self.entries.lock().expect("source icon cache");
        entries.retain(|(key, _)| key != &bundle_id);
        entries.push_front((
            bundle_id,
            CacheEntry {
                icon,
                resolved_at: Instant::now(),
            },
        ));
        entries.truncate(MAX_CACHE_ENTRIES);
    }
}

fn valid_package_id(value: &str) -> bool {
    let len = value.len();
    if !(3..=255).contains(&len) || !value.contains('.') {
        return false;
    }
    value.split('.').all(|part| {
        !part.is_empty()
            && part.len() <= 63
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    })
}

#[cfg(target_os = "macos")]
fn resolve_desktop(bundle_id: &str) -> Option<UiSourceAppIcon> {
    use std::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    const MAX_TIFF_BYTES: usize = 2 * 1024 * 1024;
    const DECODE_BUDGET_MB: u32 = 8;

    autoreleasepool(|_| {
        let bundle_id = NSString::from_str(bundle_id);
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let path = unsafe { workspace.URLForApplicationWithBundleIdentifier(&bundle_id) }
            .and_then(|url| unsafe { url.path() })?;
        let image = unsafe { workspace.iconForFile(&path) };
        let data = unsafe { image.TIFFRepresentation() }?;
        let len = data.length();
        if len == 0 || len > MAX_TIFF_BYTES {
            return None;
        }
        let mut bytes = vec![0_u8; len];
        let pointer = NonNull::new(bytes.as_mut_ptr().cast())?;
        unsafe { data.getBytes_length(pointer, len) };
        let thumbnail = copypaste_core::thumbnail_png(&bytes, DECODE_BUDGET_MB).ok()?;
        Some(UiSourceAppIcon::from_png(
            thumbnail.png,
            thumbnail.width,
            thumbnail.height,
        ))
    })
}

/// Resolve the icon for a Windows executable by its image name.
///
/// The image name (`chrome.exe`) is all an item carries — the path was dropped
/// at capture time to avoid leaking the local user name (I-9). This function
/// recovers the path transiently (App Paths registry, then System32), extracts
/// the shell icon, and discards the path. The icon PNG is what crosses back.
#[cfg(target_os = "windows")]
fn resolve_desktop(bundle_id: &str) -> Option<UiSourceAppIcon> {
    win_icon::resolve(bundle_id)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn resolve_desktop(_bundle_id: &str) -> Option<UiSourceAppIcon> {
    None
}

#[cfg(target_os = "windows")]
mod win_icon {
    use std::ffi::c_void;
    use std::io::Cursor;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::{mem, ptr};

    use image::{ImageBuffer, ImageFormat, RgbaImage};
    use windows_sys::Win32::Foundation::MAX_PATH;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    type HIcon = *mut c_void;

    use crate::model::UiSourceAppIcon;

    pub(super) fn resolve(image_name: &str) -> Option<UiSourceAppIcon> {
        let path = find_exe(image_name)?;
        let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();

        unsafe {
            let mut sfi: SHFILEINFOW = mem::zeroed();
            let ok = SHGetFileInfoW(
                path_wide.as_ptr(),
                0,
                &mut sfi,
                mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if ok == 0 {
                return None;
            }
            let icon = sfi.hIcon;
            let result = hicon_to_png(icon);
            DestroyIcon(icon);
            result
        }
    }

    fn find_exe(name: &str) -> Option<PathBuf> {
        if let Some(p) = app_paths(name) {
            return Some(p);
        }
        let root = std::env::var("SystemRoot").ok()?;
        let p = PathBuf::from(&root).join("System32").join(name);
        p.exists().then_some(p)
    }

    fn app_paths(name: &str) -> Option<PathBuf> {
        let subkey = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{name}\0");
        let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();

        unsafe {
            let mut hkey = ptr::null_mut();
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                subkey_wide.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            ) != 0
            {
                return None;
            }
            let mut buf = [0u16; MAX_PATH as usize];
            let mut size = (buf.len() * 2) as u32;
            let mut kind = 0u32;
            let ok = RegQueryValueExW(
                hkey,
                ptr::null(),
                ptr::null_mut(),
                &mut kind,
                buf.as_mut_ptr().cast(),
                &mut size,
            );
            RegCloseKey(hkey);
            if ok != 0 || kind != REG_SZ {
                return None;
            }
            let len = (size as usize / 2).saturating_sub(1);
            let s = String::from_utf16(&buf[..len]).ok()?;
            let p = PathBuf::from(s.trim_matches('"'));
            p.exists().then_some(p)
        }
    }

    unsafe fn hicon_to_png(icon: HIcon) -> Option<UiSourceAppIcon> {
        let mut info: ICONINFO = mem::zeroed();
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }

        let mut bmp: BITMAP = mem::zeroed();
        let got = GetObjectW(
            info.hbmColor as _,
            mem::size_of::<BITMAP>() as i32,
            (&raw mut bmp).cast(),
        );

        if got == 0 || bmp.bmWidth == 0 || bmp.bmHeight == 0 {
            DeleteObject(info.hbmColor as _);
            DeleteObject(info.hbmMask as _);
            return None;
        }

        let w = bmp.bmWidth as u32;
        let h = bmp.bmHeight as u32;

        let hdc = CreateCompatibleDC(ptr::null_mut());
        let mut bi: BITMAPINFO = mem::zeroed();
        bi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w as i32;
        bi.bmiHeader.biHeight = -(h as i32);
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;

        let mut pixels = vec![0u8; (w * h * 4) as usize];
        GetDIBits(
            hdc,
            info.hbmColor,
            0,
            h,
            pixels.as_mut_ptr().cast(),
            &mut bi,
            DIB_RGB_COLORS,
        );

        DeleteDC(hdc);
        DeleteObject(info.hbmColor as _);
        DeleteObject(info.hbmMask as _);

        let all_zero_alpha = pixels.chunks_exact(4).all(|px| px[3] == 0);
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            if all_zero_alpha {
                px[3] = 255;
            }
        }

        let img: RgbaImage = ImageBuffer::from_raw(w, h, pixels)?;
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .ok()?;
        Some(UiSourceAppIcon::from_png(buf, w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_ids_are_strictly_bounded() {
        assert!(valid_package_id("com.example.Writer"));
        assert!(valid_package_id("org.mozilla.firefox"));
        assert!(!valid_package_id("/Applications/Writer.app"));
        assert!(!valid_package_id("file:///tmp/icon.png"));
        assert!(!valid_package_id("writer"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_resolves_a_system_executable_icon() {
        let cache = SourceAppIconCache::default();
        assert!(
            cache.resolve_desktop("cmd.exe").is_some(),
            "cmd.exe is in System32 and must have an icon"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_returns_none_for_an_unknown_executable() {
        let cache = SourceAppIconCache::default();
        assert!(cache.resolve_desktop("not_an_app_at_all.exe").is_none());
    }

    /// DMY-158 blocker 2: the icon resolution path must not regress the poll
    /// cadence. A cold resolve is a registry lookup + SHGetFileInfoW + GDI +
    /// PNG encode; the cache makes a second resolve near-free.
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "drives the real Windows shell"]
    fn icon_resolution_is_bounded_and_the_cache_is_near_free() {
        const ROUNDS: usize = 20;
        let cache = SourceAppIconCache::default();

        let started = std::time::Instant::now();
        assert!(
            cache.resolve_desktop("cmd.exe").is_some(),
            "cold resolve must succeed"
        );
        let cold = started.elapsed();

        let mut cached = Vec::with_capacity(ROUNDS);
        for _ in 0..ROUNDS {
            let started = std::time::Instant::now();
            let _ = cache.resolve_desktop("cmd.exe");
            cached.push(started.elapsed().as_micros());
        }
        cached.sort_unstable();
        let p95 = cached[cached.len() * 95 / 100];

        println!("icon cold={}us; cached p95={}us", cold.as_micros(), p95);
        assert!(
            cold.as_millis() < 500,
            "cold icon resolve took {}ms; must fit in one poll period",
            cold.as_millis()
        );
        assert!(
            p95 < 100,
            "cached icon resolve took {}us; must be near-free",
            p95
        );
    }

    #[test]
    fn cache_is_bounded_and_reuses_a_resolved_icon() {
        let cache = SourceAppIconCache::default();
        let icon = UiSourceAppIcon::from_png(vec![1, 2, 3], 1, 1);
        assert!(cache
            .resolve_with("com.example.writer", |_| Some(icon.clone()))
            .is_some());
        assert!(cache.resolve_with("com.example.writer", |_| None).is_some());
        for index in 0..MAX_CACHE_ENTRIES + 4 {
            let bundle_id = format!("com.example.app{index}");
            let _ = cache.resolve_with(&bundle_id, |_| Some(icon.clone()));
        }
        assert_eq!(
            cache.entries.lock().expect("source icon cache").len(),
            MAX_CACHE_ENTRIES
        );
    }
}
