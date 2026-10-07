This update improves Android capture setup and automatic synchronization between paired devices.

- Retry Android capture setup from onboarding and Settings.
- Synchronize paired devices automatically and show current sync activity in the shared interface.
- Encrypt stored pairing keys and peer metadata with a device-specific key.
- Refresh the shared Settings and History controls.
- Correct update detection for Homebrew-installed macOS applications.

Shizuku is used only to apply Android setup grants and can be removed afterward. Android 13 and later can still request temporary system log access when the capture process restarts; this is separate from the Shizuku permission.

Optional modules are selected and installed from the first-party marketplace in Settings. Module cards show application and system version requirements; incompatible modules remain visible with installation disabled. OCR engines and models are not bundled with the application.

Application exclusions on macOS use observed app activity; background copies can bypass them. Empty files and multiple-file clipboard selections remain unsupported.

Cloud Sync is not part of this release.
