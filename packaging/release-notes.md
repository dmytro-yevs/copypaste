Add Linux release packages and desktop integrations.

- Build AppImage, Debian, and RPM packages for Linux x86_64 and ARM64.
- Add GNOME/KDE X11 and native Wayland clipboard, source identity, privacy controls, and Quick Paste integration. Wayland requires the matching compositor integration and keyboard permission.
- Use the desktop Secret Service for device keys and preserve encrypted history across restarts and updates.
- Verify portable AppImage updates and use the authenticated system package manager for Debian/RPM updates.
- Add Linux packages for optional OCR, Semantic Search, and Supabase modules. Keep the legacy signed marketplace catalog available for older clients.
- Report screenshot blocking as unavailable on Linux.
- Complete the macOS plugin migration to Swift Package Manager.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application.

Cloud Sync is not part of this release.
