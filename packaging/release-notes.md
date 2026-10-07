This update repairs Android clipboard capture in the optimized release APK.

- Preserve the callback method Rust uses to read and save clipboard content. Release optimization previously removed it, preventing Full capture verification from completing.
- Verify the capture callback and native ingestion methods in every production Android APK before publication.

Shizuku is used only to apply Android setup grants and can be removed afterward. Android 13 and later can still request temporary system log access when the capture process restarts; this is separate from the Shizuku permission.

Optional modules are selected and installed from the first-party marketplace in Settings. OCR engines and models are not bundled with the application.

Application exclusions on macOS use observed app activity; background copies can bypass them. Empty files and multiple-file clipboard selections remain unsupported.

Cloud Sync is not part of this release.
