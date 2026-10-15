Fix desktop window restoration and add an experimental Linux RPM download.

- Restore the desktop interface and navigation when a native window focus event follows a hidden-window memory timeout, including reopening from the macOS Dock.
- Show the macOS menu bar icon without a redundant CopyPaste text label.
- Fix macOS launch handling while preserving queued pairing links.
- Add a signed experimental x86_64 RPM alongside AppImage and Debian downloads. Require Fedora 44 DNF installation, dependency resolution, file integrity, library loading, removal, and reinstallation evidence before publication.
- Resolve installed Linux plugin libraries relative to their shared library directory.
- Keep the unused desktop JVM backend out of Linux packages; Android JNI remains available.
- Refresh the optional OCR, Semantic Search, and Supabase packages to 0.1.16 for CopyPaste 1.0.26.
- Preserve the fully qualified Ubuntu 24.04 GNOME 46 X11 release scope. ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application.

Cloud Sync is not part of this release.
