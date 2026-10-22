Improve nearby pairing, transfer performance, and download controls in CopyPaste 1.0.33.

- Connect nearby devices through a confirmation code on both devices, with typed pairing status, bounded requests, and system notifications for incoming requests.
- Show device type and connection state consistently in device controls and settings, and start device observation when the runtime becomes ready.
- Use a compact transfer manager with source and destination device identity, event-driven speed, a perimeter progress indicator on the toolbar button, and clear visual hierarchy.
- Remove an individual completed transfer from its history without deleting its saved file; completed-history clearing keeps the same preservation contract.
- Stream file blocks in one authenticated transfer instead of JSON byte arrays and one request per block. File transfers use protocol v9 and require both paired devices to update to CopyPaste 1.0.33 before they can synchronize.
- Use an accelerated cryptography backend for the existing authenticated Noise protocol.
- Keep pairing, notification, transfer, and Windows daemon-start behavior behind their existing typed adapters across supported platforms.
- Refresh OCR, Semantic Search, and Supabase packages to 0.1.24 for CopyPaste 1.0.33.

Paired devices must use this release's file-transfer protocol to exchange files. Optional modules remain available from Settings; OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the qualified Ubuntu 24.04 GNOME 46 X11 x86_64 scope. The signed RPM has Fedora 44 installation evidence; ARM64, KDE, Wayland, and full RPM desktop parity remain experimental.
