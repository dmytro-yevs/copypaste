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

/// Windows has no source-application icon, deliberately (ADR-0013, DMY-158).
///
/// A Windows item carries a process image name — `chrome.exe` — and no path:
/// the path names the local user (I-9) and is dropped where attribution is
/// resolved. Every Win32 route from a name to an icon resolves a path first,
/// and the one that does not, `SHGFI_USEFILEATTRIBUTES`, returns the same
/// generic executable icon for every application. Rows keep their semantic
/// icon rather than being given a wrong one.
#[cfg(not(target_os = "macos"))]
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

    /// The refusal is the decision, not an unfinished branch: a later "fix"
    /// that answers with the generic executable icon would put one icon on
    /// every Windows row and call it attribution.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_has_no_source_app_icon_and_says_so_by_answering_nothing() {
        let cache = SourceAppIconCache::default();
        for image_name in ["chrome.exe", "1password.exe", "com.example.writer"] {
            assert!(cache.resolve_desktop(image_name).is_none(), "{image_name}");
        }
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
