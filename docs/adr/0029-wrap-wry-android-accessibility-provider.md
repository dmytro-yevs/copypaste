# ADR-0029: Wrap Wry's Android accessibility provider

**Status:** accepted · 2026-08-28

## Decision

Wrap the provider returned by `RustWebView.getAccessibilityNodeProvider()` and
change only the marked API 33 pairing backdrop node. Forward every other node,
query, action and extra-data request to Chromium.

`AccessibilityDelegateCompat` was rejected after it failed to intercept this
WebView path. Android defines `getAccessibilityNodeProvider()` as the root of a
virtual accessibility hierarchy, and Chromium implements the WebView hierarchy
there. Chromium also maps an HTML `id` to `viewIdResourceName`, which gives the
pairing-only marker without naming or hiding its descendants.

Wry generates the final `RustWebView`, so Gradle passes the tracked extension
through Wry's `WRY_RUSTWEBVIEW_CLASS_EXTENSION` hook. The portable wiring gate
binds the Gradle input, Wry override and frontend/native marker.

## Maintenance

`tauri android init` may overwrite `BuildTask.kt`, and a Wry upgrade may rename
the template hook. Either change must update the extension and wiring test in
the same commit; missing native evidence remains a release failure.

Primary sources:

- [Android provider contract](https://developer.android.com/reference/android/view/accessibility/AccessibilityNodeProvider)
  and [AOSP source](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/view/accessibility/AccessibilityNodeProvider.java).
- [Chromium Android accessibility](https://chromium.googlesource.com/chromium/src/+/refs/tags/139.0.7227.0/docs/accessibility/browser/android.md)
  and [HTML-id mapping](https://chromium.googlesource.com/chromium/src/+/2cbfe89d4f5ccc28997de5066fa1705b89fca6d5/content/browser/accessibility/web_contents_accessibility_android.cc).
- [Wry 0.55.1 code generation](https://github.com/tauri-apps/wry/blob/a5bf203a1c8dbb3583588382538d6521655222a8/build.rs#L38-L80)
  and [RustWebView template](https://github.com/tauri-apps/wry/blob/a5bf203a1c8dbb3583588382538d6521655222a8/src/android/kotlin/RustWebView.kt).
