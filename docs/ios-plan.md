# iOS Support — Planning Notes

## Status: Future (Phase 6)

## Challenges

**Clipboard access on iOS is severely restricted:**
- iOS 16+: `UIPasteboard.general` shows a system banner "App pasted from..." every time clipboard is read
- iOS 14+: No background clipboard monitoring — app must be in foreground
- No `ClipboardManager` equivalent; polling is only option
- `NSPasteboard` (macOS) has no iOS equivalent — use `UIPasteboard`

**No persistent background daemon:**
- iOS kills background processes aggressively
- Must use Share Extension or Shortcuts integration to capture items from other apps
- Local database (SQLCipher) can still be used via SQLite on iOS

## Viable Architecture for iOS

1. **Share Extension** — user explicitly shares text to CopyPaste app → encrypted + stored
2. **Shortcuts integration** — iOS Shortcuts can invoke the app to store current clipboard
3. **iCloud Keychain** — for key storage (replaces macOS Keychain API)
4. **CloudKit sync** — iOS-native alternative to relay server

## UniFFI Considerations

`copypaste-android` pattern applies to iOS:
- Crate: `copypaste-ios` (cdylib with `staticlib` for XCFramework)
- Bindings: `uniffi-bindgen-swift` generates Swift wrappers
- No clipboard monitoring in Rust — only encryption/storage/detection

## Minimum iOS Version

iOS 16+ (for reasonable clipboard + background behavior)

## Build Pipeline

```bash
cargo build --target aarch64-apple-ios --release
uniffi-bindgen-swift crates/copypaste-ios/uniffi/copypaste_ios.udl --out-dir ios/Sources/CopyPaste/
xcodebuild archive ...
```

## Estimated Effort

- UniFFI crate: 1 day
- Swift app: 3-5 days
- Share Extension: 2 days
- iCloud Keychain integration: 1 day
- Total: ~1-2 weeks
