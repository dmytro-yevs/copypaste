This update improves mobile navigation, clipboard inspection, and desktop updates.

- Open SMS module access setup in a drawer and refresh permission state when the application resumes.
- Reuse build dependency caches across CI and release runs, and avoid the slow Windows Flutter SDK archive.
- Match the Android Telegram navigation geometry with compact labels, glass backgrounds, animated selection, page swipes, hold-and-drag selection, and repeat activation to scroll to the top.
- Keep navigation targets and text sizes consistent across Android, macOS, and Windows, including accessibility text enlargement.
- Improve clipboard inspection and OCR interactions, and keep button borders consistent with the shared theme.
- Restart the application after desktop updates, including native macOS relaunch support.
- Preserve macOS capture-protected frame colors and color-space metadata.
- Correct Windows file-drop names and synchronize the Supabase module dependency lock with the release version.

Optional modules are selected and installed from the first-party marketplace in Settings. OCR engines and models are not bundled with the application.

Cloud Sync is not part of this release.
