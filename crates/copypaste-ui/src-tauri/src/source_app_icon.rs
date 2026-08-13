//! Native source-application icons, converted before they reach the WebView.

#![allow(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::model::UiSourceAppIcon;

const MAX_CACHE_ENTRIES: usize = 64;
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
#[cfg(target_os = "windows")]
const MAX_ICON_EDGE: u32 = 512;
#[cfg(target_os = "windows")]
const MAX_ICON_BYTES: usize = 512 * 1024;

enum CacheEntry {
    Resolved { icon: UiSourceAppIcon, at: Instant },
    Missing { at: Instant },
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
        let key = cache_key(bundle_id);
        if let Some(hit) = self.cached(&key) {
            return hit;
        }
        let icon = resolver(bundle_id);
        self.insert(key, icon.clone());
        icon
    }

    /// Returns `Some(Some(icon))` on a positive hit, `Some(None)` on a negative
    /// hit (known miss), and `None` on a cache miss.
    fn cached(&self, key: &str) -> Option<Option<UiSourceAppIcon>> {
        let mut entries = self.entries.lock().expect("source icon cache");
        let index = entries.iter().position(|(k, _)| k == key)?;
        let entry = entries.remove(index).expect("entry index is valid");
        match &entry.1 {
            CacheEntry::Resolved { icon, at } => {
                if at.elapsed() >= CACHE_TTL {
                    return None;
                }
                let icon = icon.clone();
                entries.push_front(entry);
                Some(Some(icon))
            }
            CacheEntry::Missing { at } => {
                if at.elapsed() >= CACHE_TTL {
                    return None;
                }
                entries.push_front(entry);
                Some(None)
            }
        }
    }

    fn insert(&self, key: String, icon: Option<UiSourceAppIcon>) {
        let mut entries = self.entries.lock().expect("source icon cache");
        entries.retain(|(k, _)| k != &key);
        let entry = match icon {
            Some(icon) => CacheEntry::Resolved {
                icon,
                at: Instant::now(),
            },
            None => CacheEntry::Missing { at: Instant::now() },
        };
        entries.push_front((key, entry));
        entries.truncate(MAX_CACHE_ENTRIES);
    }
}

fn cache_key(bundle_id: &str) -> String {
    if cfg!(target_os = "windows") || bundle_id.ends_with(".exe") {
        bundle_id.to_ascii_lowercase()
    } else {
        bundle_id.to_owned()
    }
}

fn valid_package_id(value: &str) -> bool {
    let len = value.len();
    if !(3..=255).contains(&len) {
        return false;
    }
    // Windows image names: `chrome.exe`, `proton pass.exe`
    if value
        .get(value.len().saturating_sub(4)..)
        .is_some_and(|ext| ext.eq_ignore_ascii_case(".exe"))
    {
        let stem = &value[..value.len() - 4];
        return !stem.is_empty()
            && !stem.contains(['\\', '/', ':'])
            && stem.bytes().all(|b| !b.is_ascii_control());
    }
    // Bundle identifiers: `com.example.App`
    if !value.contains('.') {
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
/// recovers the path transiently (HKCU/HKLM App Paths, then System32),
/// extracts the shell icon, and discards the path. The icon PNG crosses back.
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
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_EXPAND_SZ, REG_SZ,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    type HIcon = *mut c_void;

    use super::{MAX_ICON_BYTES, MAX_ICON_EDGE};
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

    /// Search App Paths under HKCU then HKLM, and probe both the native and
    /// the WOW64 registry view on each, so per-user installs (Vivaldi, VS Code)
    /// and 32-bit apps registered under `Wow6432Node` are found.
    fn app_paths(name: &str) -> Option<PathBuf> {
        let subkey = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{name}\0");
        let subkey_wide: Vec<u16> = subkey.encode_utf16().collect();

        for &root in &[HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            for &extra_flags in &[0u32, KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
                if let Some(p) = read_app_path(root, &subkey_wide, KEY_READ | extra_flags) {
                    return Some(p);
                }
            }
        }
        None
    }

    fn read_app_path(root: isize, subkey: &[u16], flags: u32) -> Option<PathBuf> {
        unsafe {
            let mut hkey = ptr::null_mut();
            if RegOpenKeyExW(root, subkey.as_ptr(), 0, flags, &mut hkey) != 0 {
                return None;
            }
            let result = read_default_value(hkey);
            RegCloseKey(hkey);
            let raw = result?;
            let expanded = expand_env(&raw);
            let p = PathBuf::from(expanded.trim_matches('"').trim());
            p.exists().then_some(p)
        }
    }

    /// Read the default value (`lpValueName = NULL`), accepting both `REG_SZ`
    /// and `REG_EXPAND_SZ`, and dynamically sizing the buffer on
    /// `ERROR_MORE_DATA`.
    unsafe fn read_default_value(hkey: *mut c_void) -> Option<String> {
        let mut size = (MAX_PATH as usize * 2) as u32;
        let mut buf: Vec<u16> = vec![0; size as usize / 2];
        let mut kind = 0u32;
        let mut rc = RegQueryValueExW(
            hkey,
            ptr::null(),
            ptr::null_mut(),
            &mut kind,
            buf.as_mut_ptr().cast(),
            &mut size,
        );
        if rc == 234 {
            // ERROR_MORE_DATA
            buf.resize(size as usize / 2 + 1, 0);
            rc = RegQueryValueExW(
                hkey,
                ptr::null(),
                ptr::null_mut(),
                &mut kind,
                buf.as_mut_ptr().cast(),
                &mut size,
            );
        }
        if rc != 0 || !matches!(kind, REG_SZ | REG_EXPAND_SZ) {
            return None;
        }
        let chars = size as usize / 2;
        let len = if chars > 0 && buf.get(chars - 1) == Some(&0) {
            chars - 1
        } else {
            chars
        };
        String::from_utf16(&buf[..len]).ok()
    }

    fn expand_env(value: &str) -> String {
        if !value.contains('%') {
            return value.to_owned();
        }
        let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let needed = unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), ptr::null_mut(), 0) };
        if needed == 0 {
            return value.to_owned();
        }
        let mut buf = vec![0u16; needed as usize];
        let written = unsafe { ExpandEnvironmentStringsW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
        if written == 0 || written > needed {
            return value.to_owned();
        }
        let len = (written as usize).saturating_sub(1);
        String::from_utf16(&buf[..len]).unwrap_or_else(|_| value.to_owned())
    }

    unsafe fn hicon_to_png(icon: HIcon) -> Option<UiSourceAppIcon> {
        let mut info: ICONINFO = mem::zeroed();
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }

        // Monochrome icons have hbmColor == NULL and a double-height mask.
        if info.hbmColor.is_null() {
            DeleteObject(info.hbmMask as _);
            return None;
        }

        let mut bmp: BITMAP = mem::zeroed();
        let got = GetObjectW(
            info.hbmColor as _,
            mem::size_of::<BITMAP>() as i32,
            (&raw mut bmp).cast(),
        );

        if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
            DeleteObject(info.hbmColor as _);
            DeleteObject(info.hbmMask as _);
            return None;
        }

        let w = bmp.bmWidth as u32;
        let h = bmp.bmHeight as u32;

        if w > MAX_ICON_EDGE || h > MAX_ICON_EDGE {
            DeleteObject(info.hbmColor as _);
            DeleteObject(info.hbmMask as _);
            return None;
        }

        let pixel_count = (w as usize).checked_mul(h as usize)?;
        let byte_count = pixel_count.checked_mul(4)?;

        let hdc = CreateCompatibleDC(ptr::null_mut());
        if hdc.is_null() {
            DeleteObject(info.hbmColor as _);
            DeleteObject(info.hbmMask as _);
            return None;
        }

        let mut bi: BITMAPINFO = mem::zeroed();
        bi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w as i32;
        bi.bmiHeader.biHeight = -(h as i32);
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;

        let mut pixels = vec![0u8; byte_count];
        let rows = GetDIBits(
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

        if rows == 0 {
            return None;
        }

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
        if buf.len() > MAX_ICON_BYTES {
            return None;
        }
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

    #[test]
    fn windows_image_names_with_spaces_are_accepted() {
        assert!(valid_package_id("chrome.exe"));
        assert!(valid_package_id("proton pass.exe"));
        assert!(valid_package_id("sticky password.exe"));
        assert!(valid_package_id("robotaskbaricon-x64.exe"));
        assert!(!valid_package_id(".exe"));
        assert!(!valid_package_id(""));
        assert!(!valid_package_id(r"C:\Apps\chrome.exe"));
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

    #[test]
    fn negative_cache_prevents_repeated_resolution() {
        let cache = SourceAppIconCache::default();
        let mut calls = 0u32;
        assert!(cache
            .resolve_with("com.example.missing", |_| {
                calls += 1;
                None
            })
            .is_none());
        assert_eq!(calls, 1);
        assert!(cache
            .resolve_with("com.example.missing", |_| {
                calls += 1;
                None
            })
            .is_none());
        assert_eq!(calls, 1, "a second resolve called the resolver again");
    }

    #[test]
    fn cache_keys_are_case_insensitive_on_windows_image_names() {
        let cache = SourceAppIconCache::default();
        let icon = UiSourceAppIcon::from_png(vec![1, 2, 3], 1, 1);
        assert!(cache
            .resolve_with("chrome.exe", |_| Some(icon.clone()))
            .is_some());
        assert!(
            cache
                .resolve_with("Chrome.exe", |_| panic!("should hit cache"))
                .is_some(),
            "case variant must hit cache"
        );
    }

    #[test]
    fn cache_eviction_drops_oldest_entry() {
        let cache = SourceAppIconCache::default();
        let icon = UiSourceAppIcon::from_png(vec![1, 2, 3], 1, 1);
        cache.resolve_with("com.example.first", |_| Some(icon.clone()));
        for i in 0..MAX_CACHE_ENTRIES {
            let id = format!("com.example.evict{i}");
            cache.resolve_with(&id, |_| Some(icon.clone()));
        }
        let mut calls = 0u32;
        cache.resolve_with("com.example.first", |_| {
            calls += 1;
            Some(icon.clone())
        });
        assert_eq!(calls, 1, "evicted entry should re-resolve");
    }
}
