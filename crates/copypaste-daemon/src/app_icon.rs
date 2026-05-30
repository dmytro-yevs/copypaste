//! App-icon extraction and in-memory cache.
//!
//! Given a bundle identifier (e.g. `"com.google.Chrome"`), this module resolves
//! the app on disk via `NSWorkspace`, renders its icon into a 32×32 PNG, and
//! returns the result as a base64 string.  Results are cached in a
//! `Mutex<HashMap>` so that AppKit is only called once per bundle identifier per
//! daemon lifetime.
//!
//! `None` in the cache means "already tried, no icon found" — we never re-query
//! an absent app on every request.
//!
//! Compiled only on macOS; on other platforms the public surface is a no-op.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Process-global icon cache.  `None` value = already queried, no icon found.
static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Return the cached result for `bundle_id`, or `None` if not yet cached.
/// Returns `Some(None)` when cached as a negative result (app not found).
fn cache_get(bundle_id: &str) -> Option<Option<String>> {
    let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.get(bundle_id).cloned()
}

fn cache_set(bundle_id: &str, value: Option<String>) {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(bundle_id.to_string(), value);
}

/// Resolve `bundle_id` to a 32×32 PNG, base64-encoded.
///
/// Returns `None` when the app is not installed or icon extraction fails.
/// The result is cached in-memory so AppKit is called at most once per id.
pub fn get_app_icon_base64(bundle_id: &str) -> Option<String> {
    // Fast path: already cached.
    if let Some(cached) = cache_get(bundle_id) {
        return cached;
    }

    let result = extract_icon(bundle_id);
    cache_set(bundle_id, result.clone());
    result
}

/// Platform-specific icon extraction.  Returns base64 PNG on macOS; always
/// `None` on other platforms so the IPC method returns `null` cleanly.
#[cfg(target_os = "macos")]
fn extract_icon(bundle_id: &str) -> Option<String> {
    use base64::Engine as _;
    use objc2::rc::autoreleasepool;
    // ClassType provides the `alloc()` associated function used below.
    use objc2::ClassType as _;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSGraphicsContext,
        NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    // SAFETY: NSWorkspace, NSImage, NSBitmapImageRep, and NSGraphicsContext all
    // have `InteriorMutable` mutability in objc2-app-kit 0.2, meaning they are
    // allocable and usable from any thread.  We drain the autorelease pool at
    // the end of the closure to avoid leaking autoreleased Cocoa objects.
    unsafe {
        autoreleasepool(|_pool| {
            let ws = NSWorkspace::sharedWorkspace();
            let bundle_id_ns = NSString::from_str(bundle_id);

            // Resolve the bundle ID to a filesystem path.
            let app_url = ws.URLForApplicationWithBundleIdentifier(&bundle_id_ns)?;
            let path = app_url.path()?;

            // Retrieve the icon for the application path.  NSWorkspace returns a
            // vector NSImage (usually backed by an .icns) that can be rendered at
            // any size.
            let icon = ws.iconForFile(&path);

            // Target raster size: 32×32 pts at 1x.
            const SIZE: f64 = 32.0;
            let target_size = NSSize::new(SIZE, SIZE);

            // Allocate an RGBA 32×32 bitmap image rep that we will use as a
            // drawing destination.
            let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),  // auto-allocate pixel planes
                SIZE as isize,         // pixelsWide
                SIZE as isize,         // pixelsHigh
                8,                     // bitsPerSample
                4,                     // samplesPerPixel (RGBA)
                true,                  // hasAlpha
                false,                 // isPlanar
                objc2_app_kit::NSDeviceRGBColorSpace,
                0,                     // bytesPerRow  — 0 = auto
                0,                     // bitsPerPixel — 0 = auto
            )?;

            // Create an NSGraphicsContext backed by our bitmap and push it as the
            // current context so that drawing methods write into `rep`.
            // graphicsContextWithBitmapImageRep: returns Option — None would mean
            // the bitmap rep is malformed; we treat that as a missing icon.
            let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
            NSGraphicsContext::setCurrentContext(Some(&ctx));

            // Scale and draw the icon into the 32×32 rect.
            let dest_rect = NSRect::new(NSPoint::new(0.0, 0.0), target_size);
            // fromRect: NSZeroRect means "use the full source image".
            let zero_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
            icon.drawInRect_fromRect_operation_fraction(
                dest_rect,
                zero_rect,
                NSCompositingOperation::SourceOver,
                1.0,
            );

            // Restore the previous (nil) context.
            NSGraphicsContext::setCurrentContext(None);

            // Encode the bitmap as PNG.
            let props = NSDictionary::dictionary();
            let png_data =
                rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props)?;

            let bytes = png_data.bytes();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            Some(encoded)
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn extract_icon(_bundle_id: &str) -> Option<String> {
    None
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Clear the icon cache.  Only available in tests so that independent test
/// cases start from a known empty state.
#[cfg(test)]
pub fn clear_cache_for_test() {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_negative_result_is_not_retried() {
        clear_cache_for_test();
        // Query a bundle ID that will never exist on any machine.
        let result = get_app_icon_base64("com.fake.app.that.does.not.exist");
        assert!(result.is_none(), "non-existent app should return None");
        // The second call must read from cache (same None).
        let result2 = get_app_icon_base64("com.fake.app.that.does.not.exist");
        assert!(result2.is_none());
    }
}
