Fix spoiler consistency and Quick Paste interactions.

- Match spoiler masks to the actual text, image preview, and inspector metadata dimensions, including scaled text.
- Share temporary spoiler reveals across History, Quick Paste, and their inspectors through revisioned local runtime state. Keep persisted secret choices synchronized and reject stale visibility responses.
- Replace eye controls with lock icons and explicit Add spoiler / Remove spoiler action labels.
- Remove borders from Quick Paste menu buttons in every interaction state.
- Preserve recent and pinned list scroll positions while opening or closing the Quick Paste inspector. Keep the menu width stable during native expansion and use the shared entrance animation.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.19 for CopyPaste 1.0.28.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
