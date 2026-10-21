Manage file imports and device transfers consistently in CopyPaste 1.0.32.

- Keep file transfers running when Quick Paste closes, with shared progress and errors across History, inspectors, and Quick Paste.
- Add a downloads manager with byte progress, source and destination devices, timestamps, persistent history, retry, cancellation, and saved-file links.
- Reuse completed downloads when a clip is activated again. Clearing transfer history preserves files, clips, and active transfers.
- Make selected files available immediately by registering their original location and streaming bytes only on request. An unavailable or changed original produces an actionable error.
- Separate Quick Paste status from keyboard shortcuts and display the failure reason above clip metadata.
- Report capture running, paused, stopped, and unavailable states consistently in the header and tray, and offer recovery for stopped Android screenshot capture.
- Keep Quick Paste action failures visible and refresh OCR availability when module or runtime state changes.
- Keep device cards compact with inline connection latency and shared typography.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.23 for CopyPaste 1.0.32.

Paired devices must use this release's file-transfer protocol to exchange files. Optional modules remain available from Settings; OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
