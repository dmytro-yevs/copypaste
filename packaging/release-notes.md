This update adds automatic Android screenshot capture and improves the shared interface, SMS module setup, and paired-device synchronization.

- Save new Android screenshots to encrypted History automatically, with a Settings switch enabled by default.
- Keep screenshot capture independent of Shizuku and preserve its setting across restarts.
- Unify Android background capture and SMS module permission setup.
- Refresh shared navigation, History inspectors, device controls, and application themes.
- Improve paired-device connection and synchronization handling.

Screenshot capture requires access to all photos and Android notification permission. Existing screenshots are not imported. Private mode, application exclusions, and existing capture limits continue to apply.

Shizuku is used only to apply Android setup grants and can be removed afterward. Android 13 and later can still request temporary system log access when the clipboard capture process restarts; this is separate from the Shizuku permission.

Optional modules are selected and installed from the first-party marketplace in Settings. OCR engines and models are not bundled with the application.

Application exclusions on macOS use observed app activity; background copies can bypass them. Empty files and multiple-file clipboard selections remain unsupported.

Cloud Sync is not part of this release.
