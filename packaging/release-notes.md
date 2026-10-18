Improve Quick Paste and modernize Linux platform detection in CopyPaste 1.0.30.

- Open Quick Paste directly from the macOS tray with the shared native presentation path.
- Prepare synchronized files from another device when selected in Quick Paste, show progress measured from written bytes, and reuse a recent local copy.
- Show file names above their original paths, keeping both ends visible when a path is too long.
- Show progress on the Download button and offer Cancel on hover while keeping icon-only buttons square; failed transfers can be retried.
- Copy remote files as native file payloads instead of copying a path from the sending device.
- Probe Linux clipboard backends at runtime and select them for each operation's required guarantees. Report missing source attribution and privacy enforcement explicitly.
- Expose typed capability statuses and redacted diagnostics through the CLI and Flutter integration.
- Generate Linux build, qualification, and support views from one platform manifest. Add GNOME Wayland x86_64 as an experimental target.
- Reduce unnecessary compositor builds and collect comparable CI timing evidence while preserving exact signed artifact verification, upgrade tests, and native acceptance.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.21 for CopyPaste 1.0.30.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental. GNOME Wayland does not have stable native parity evidence.
