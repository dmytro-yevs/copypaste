//! Bounded native source-application icons without a UI dependency.

#![allow(unsafe_code)]

#[cfg(target_os = "windows")]
mod gdi;
#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(target_os = "windows")]
pub mod registry;
#[cfg(target_os = "windows")]
mod win_icon;

use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

const MAX_CACHE_ENTRIES: usize = 64;
const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
pub const MAX_ICON_EDGE: u32 = 128;
pub const MAX_ICON_BYTES: usize = 32 * 1024;
const MAX_SOURCE_ICON_EDGE: u32 = 512;
const MAX_SOURCE_ICON_BYTES: usize = 512 * 1024;

/// A normalized source-application icon safe to persist and transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIcon {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

impl AppIcon {
    pub fn from_base64(png_base64: impl AsRef<str>, width: u32, height: u32) -> Option<Self> {
        let encoded = png_base64.as_ref();
        if encoded.len() > MAX_SOURCE_ICON_BYTES.div_ceil(3) * 4 {
            return None;
        }
        let png = STANDARD.decode(encoded).ok()?;
        Self::from_png(png, width, height)
    }

    pub fn from_png(png: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        if png.is_empty()
            || png.len() > MAX_SOURCE_ICON_BYTES
            || width == 0
            || height == 0
            || width > MAX_SOURCE_ICON_EDGE
            || height > MAX_SOURCE_ICON_EDGE
        {
            return None;
        }
        let mut reader = ImageReader::with_format(Cursor::new(&png), ImageFormat::Png);
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SOURCE_ICON_EDGE);
        limits.max_image_height = Some(MAX_SOURCE_ICON_EDGE);
        limits.max_alloc = Some(4 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().ok()?;
        if image.width() != width || image.height() != height {
            return None;
        }
        normalize(image)
    }
}

/// Decode and normalize a PNG whose dimensions are supplied by the image
/// itself. Desktop-entry icons do not carry trusted out-of-band dimensions,
/// unlike icons received through the IPC contract.
#[cfg(any(target_os = "linux", test))]
fn normalize_png(png: Vec<u8>) -> Option<AppIcon> {
    if png.is_empty() || png.len() > MAX_SOURCE_ICON_BYTES {
        return None;
    }
    let mut reader = ImageReader::with_format(Cursor::new(&png), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_ICON_EDGE);
    limits.max_image_height = Some(MAX_SOURCE_ICON_EDGE);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    normalize(reader.decode().ok()?)
}

fn normalize(image: DynamicImage) -> Option<AppIcon> {
    let image = if image.width() > MAX_ICON_EDGE || image.height() > MAX_ICON_EDGE {
        image.thumbnail(MAX_ICON_EDGE, MAX_ICON_EDGE)
    } else {
        image
    };
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return None;
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    (png.len() <= MAX_ICON_BYTES).then(|| AppIcon {
        png_base64: STANDARD.encode(png),
        width,
        height,
    })
}

enum CacheEntry {
    Resolved { icon: AppIcon, at: Instant },
    Missing { at: Instant },
}

#[derive(Default)]
pub struct SourceAppIconCache {
    entries: Mutex<VecDeque<(String, CacheEntry)>>,
}

impl SourceAppIconCache {
    pub fn resolve_desktop(&self, app_id: &str) -> Option<AppIcon> {
        self.resolve_with(app_id, resolve_desktop)
    }

    pub fn resolve_with(
        &self,
        app_id: &str,
        resolver: impl FnOnce(&str) -> Option<AppIcon>,
    ) -> Option<AppIcon> {
        if !valid_package_id(app_id) {
            return None;
        }
        let key = cache_key(app_id);
        if let Some(hit) = self.cached(&key) {
            return hit;
        }
        let icon = resolver(app_id);
        self.insert(key, icon.clone());
        icon
    }

    fn cached(&self, key: &str) -> Option<Option<AppIcon>> {
        let mut entries = self.entries.lock().expect("source icon cache");
        let index = entries.iter().position(|(candidate, _)| candidate == key)?;
        let entry = entries.remove(index).expect("entry index is valid");
        match &entry.1 {
            CacheEntry::Resolved { icon, at } if at.elapsed() < CACHE_TTL => {
                let icon = icon.clone();
                entries.push_front(entry);
                Some(Some(icon))
            }
            CacheEntry::Missing { at } if at.elapsed() < CACHE_TTL => {
                entries.push_front(entry);
                Some(None)
            }
            CacheEntry::Resolved { .. } | CacheEntry::Missing { .. } => None,
        }
    }

    fn insert(&self, key: String, icon: Option<AppIcon>) {
        let mut entries = self.entries.lock().expect("source icon cache");
        entries.retain(|(candidate, _)| candidate != &key);
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

fn cache_key(app_id: &str) -> String {
    if cfg!(target_os = "windows") || app_id.ends_with(".exe") {
        app_id.to_ascii_lowercase()
    } else {
        app_id.to_owned()
    }
}

pub fn valid_package_id(value: &str) -> bool {
    let len = value.len();
    if !(3..=255).contains(&len) {
        return false;
    }
    if value
        .get(value.len().saturating_sub(4)..)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(".exe"))
    {
        let stem = &value[..value.len() - 4];
        return !stem.is_empty()
            && !stem.contains(['\\', '/', ':'])
            && stem.bytes().all(|byte| !byte.is_ascii_control());
    }
    value.contains('.')
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

#[cfg(target_os = "macos")]
fn resolve_desktop(bundle_id: &str) -> Option<AppIcon> {
    use std::ptr::NonNull;

    use objc2::{rc::autoreleasepool, ClassType};
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
        NSGraphicsContext, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    const ICON_EDGE: usize = 64;

    autoreleasepool(|_| {
        let requested_bundle_id = bundle_id;
        let bundle_id = NSString::from_str(requested_bundle_id);
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let frontmost_path = unsafe {
            workspace.frontmostApplication().and_then(|application| {
                let matches = application
                    .bundleIdentifier()
                    .is_some_and(|id| id.to_string() == requested_bundle_id);
                matches
                    .then(|| application.bundleURL())
                    .flatten()
                    .and_then(|url| url.path())
            })
        };
        let path = frontmost_path.or_else(|| {
            unsafe { workspace.URLForApplicationWithBundleIdentifier(&bundle_id) }
                .and_then(|url| unsafe { url.path() })
        })?;
        let image = unsafe { workspace.iconForFile(&path) };
        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                ICON_EDGE as isize,
                ICON_EDGE as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
        }?;
        let context = unsafe { NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap) }?;
        let previous_context = unsafe { NSGraphicsContext::currentContext() };
        unsafe {
            NSGraphicsContext::setCurrentContext(Some(&context));
            image.drawInRect_fromRect_operation_fraction(
                NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(ICON_EDGE as f64, ICON_EDGE as f64),
                ),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                NSCompositingOperation::Copy,
                1.0,
            );
            context.flushGraphics();
            NSGraphicsContext::setCurrentContext(previous_context.as_deref());
        }
        let data = unsafe {
            bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )
        }?;
        let len = data.length();
        if len == 0 || len > MAX_ICON_BYTES {
            return None;
        }
        let mut png = vec![0_u8; len];
        let pointer = NonNull::new(png.as_mut_ptr().cast())?;
        unsafe { data.getBytes_length(pointer, len) };
        AppIcon::from_png(png, ICON_EDGE as u32, ICON_EDGE as u32)
    })
}

#[cfg(target_os = "windows")]
fn resolve_desktop(app_id: &str) -> Option<AppIcon> {
    win_icon::resolve(app_id)
}

#[cfg(target_os = "linux")]
fn resolve_desktop(app_id: &str) -> Option<AppIcon> {
    linux::resolve(app_id)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn resolve_desktop(_app_id: &str) -> Option<AppIcon> {
    None
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(width, height, Rgba([0x24u8, 0x65, 0xa8, 0xff]));
        let mut output = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Png)
            .unwrap();
        output
    }

    #[test]
    fn package_ids_are_strictly_bounded() {
        assert!(valid_package_id("com.example.Writer"));
        assert!(valid_package_id("org.mozilla.firefox"));
        assert!(valid_package_id("proton pass.exe"));
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
    fn declared_dimensions_cannot_bypass_decoder_limits() {
        assert!(AppIcon::from_png(png(513, 1), 32, 32).is_none());
        let encoded = "A".repeat(MAX_SOURCE_ICON_BYTES.div_ceil(3) * 4 + 1);
        assert!(AppIcon::from_base64(encoded, 32, 32).is_none());
    }

    #[test]
    fn pngs_are_normalized_to_the_shared_bounds() {
        let icon = AppIcon::from_png(png(256, 128), 256, 128).expect("a valid icon");
        assert_eq!((icon.width, icon.height), (128, 64));
        assert!(STANDARD.decode(icon.png_base64).unwrap().len() <= MAX_ICON_BYTES);
    }

    #[test]
    fn cache_is_bounded_and_reuses_a_resolved_icon() {
        let cache = SourceAppIconCache::default();
        let icon = AppIcon::from_png(png(1, 1), 1, 1).unwrap();
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
        let icon = AppIcon::from_png(png(1, 1), 1, 1).unwrap();
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
        let icon = AppIcon::from_png(png(1, 1), 1, 1).unwrap();
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
