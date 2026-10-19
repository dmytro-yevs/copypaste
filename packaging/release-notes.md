Improve file transfers, desktop controls, and clipboard lifecycle in CopyPaste 1.0.31.

- Synchronize file metadata and download original file bytes on demand, with progress, cancellation, retry, and encrypted backup support.
- Keep History and device inspector actions visible while long metadata scrolls, including compact layouts and larger text.
- Share desktop keyboard, window-menu, Quick Paste, dialog, and onboarding behavior through the platform adapters and common controls.
- Reclaim owned temporary clipboard, import, installer, and module staging files while preserving current payloads and pending updates.
- Keep dialog keyboard motion in the shared motion policy and isolate Android clipboard provider state in tests.
- Probe Linux clipboard backends at runtime and expose typed capability statuses and redacted diagnostics.
- Share Linux build and qualification targets through one platform manifest; GNOME Wayland remains experimental.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.22 for CopyPaste 1.0.31.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental. GNOME Wayland does not have stable native parity evidence.
