Improve clipboard privacy, update recovery, and delivery across supported platforms.

- Add synchronized secret/non-secret controls to the shared History and Quick Paste inspector, with stale-detail protection after remote privacy changes. Restore inspector copy, download, OCR, pin, and delete actions in Quick Paste.
- Recognize confidentiality from explicit clipboard hints instead of malformed optional metadata. Keep identical text, image, and file representations separate in History with a versioned storage migration.
- Reconnect History after an interrupted runtime stream and preserve selection across suspension and refresh.
- Prepare authenticated updates for installed modules alongside application updates, then activate compatible packages after restart while preserving preferences. Move manual module updates to the module list.
- Preserve Android SMS monitoring state on lookup failures and acknowledge messages only after successful processing. Respect observed screenshot permission boundaries and reconcile failed capture setup with persisted state.
- Clear unavailable cloud-provider credentials on sign-out, stop active synchronization after device removal, and notify sync about partially completed imports.
- Report unreadable settings and incomplete text exports. Detect corrupt semantic models and retain durable module-removal cleanup work.
- Recover temporary plaintext cleanup after restart, including locked Windows files, and preserve remote device attribution when optional metadata lookup fails.
- Deliver all Android APK variants in one build. Automatically orchestrate exact-artifact qualification, authenticate artifact transfers, and resume publication from qualified packages without rebuilding.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.18 for CopyPaste 1.0.27.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
