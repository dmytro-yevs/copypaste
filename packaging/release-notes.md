Add Linux release packages and desktop integrations.

- Release AppImage and Debian packages for Linux x86_64 on GNOME X11 (Ubuntu 24.04).
- Qualify GNOME X11 clipboard, source identity, privacy controls, and Quick Paste integration. ARM64, KDE, Wayland, and RPM implementation remains available for development, outside this release.
- Keep private and confidential values out of History when the X11 clipboard manager restores an older cached selection after its source closes.
- Use the desktop Secret Service for device keys and preserve encrypted history across restarts and updates.
- Verify portable AppImage updates and use the authenticated system package manager for Debian updates.
- Add Linux packages for optional OCR, Semantic Search, and Supabase modules. Keep the legacy signed marketplace catalog available for older clients.
- Report screenshot blocking as unavailable on Linux.
- Complete the macOS plugin migration to Swift Package Manager.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application.

Cloud Sync is not part of this release.
