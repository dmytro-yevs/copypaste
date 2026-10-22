Improve header visibility and keep icons and download progress correctly proportioned in CopyPaste 1.0.34.

- Keep drawers below the application header so its controls remain visible and usable.
- Keep mobile header actions beside the title instead of forcing a second row when more than three actions are present.
- Give the Notifications clear action the same outlined treatment as Downloads.
- Enforce a shared 1:1 aspect ratio for application icons, circular progress indicators, source application icons, color swatches, and transfer button progress, including enlarged text and stretched control layouts.
- Keep download progress circular in History and Quick Paste instead of stretching it into an oval.
- Bind OCR, Semantic Search, and Supabase module packages 0.1.25 to CopyPaste 1.0.34.

File transfers retain protocol v9 and require compatible versions on paired devices. Optional modules remain available from Settings; OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the Ubuntu 24.04 GNOME 46 X11 x86_64 release scope. RPM, ARM64, KDE, Wayland, and full RPM desktop parity retain their existing qualification limits. Native and resource qualification is determined by the release evidence, not by these UI changes or portable tests.
