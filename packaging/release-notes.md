Improve startup controls, Settings navigation, and resource lifecycle in CopyPaste 1.0.29.

- Add Start at login on macOS and Windows, and Start on device boot on Android, with platform-backed preference state.
- Open Settings with Command-Comma on macOS or Control-Comma on Windows and Android; open History from the app brand control.
- Keep History and Quick Paste responsive with bounded media caches, smaller retained summaries, and on-demand presentation resources.
- Reduce image-preview memory by streaming supported PNG thumbnails through a bounded row window and releasing macOS decoder scratch memory.
- Release OCR workers after each request and bound Android screenshot metadata scans to small batches.
- Stabilize OCR dialog layout and keep module drawer actions visible below scrolling content.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.20 for CopyPaste 1.0.29.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
