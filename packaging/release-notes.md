This update fixes Android background capture setup.

- Fix a crash when a background copy briefly focuses the capture window.
- Restore the capture setup step and verification progress after an application restart.
- Save verified setup before the optional device pairing step.
- Keep completed onboarding when runtime clipboard intake is unavailable.

Shizuku is used only to apply Android setup grants and can be removed afterward. Android 13 and later can still request temporary system log access when the capture process restarts; this is separate from the Shizuku permission.

Optional modules are selected and installed from the first-party marketplace in Settings. OCR engines and models are not bundled with the application.

Application exclusions on macOS use observed app activity; background copies can bypass them. Empty files and multiple-file clipboard selections remain unsupported.

Cloud Sync is not part of this release.
