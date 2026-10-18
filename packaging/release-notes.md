Improve Quick Paste file handling and tray access in CopyPaste 1.0.30.

- Open Quick Paste directly from the macOS tray with the shared native presentation path.
- Prepare synchronized files from another device when selected in Quick Paste, show progress measured from written bytes, and reuse a recent local copy.
- Show file names above their original paths, keeping both ends visible when a path is too long.
- Show progress on the Download button and offer Cancel on hover while keeping icon-only buttons square; failed transfers can be retried.
- Copy remote files as native file payloads instead of copying a path from the sending device.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.21 for CopyPaste 1.0.30.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
