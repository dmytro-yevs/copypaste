//! Native source-application icons, converted before they reach the WebView.

#![allow(unsafe_code)]

#[cfg(target_os = "windows")]
mod gdi;
#[cfg(target_os = "windows")]
mod registry;
#[cfg(target_os = "windows")]
mod win_icon;

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
    #[ignore = "drives the real Windows shell"]
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
