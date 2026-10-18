Modernize Linux platform detection and release qualification in CopyPaste 1.0.30.

- Probe Linux clipboard backends at runtime and select them for each operation's required guarantees. Explicitly report missing source attribution and privacy enforcement instead of treating a basic clipboard read as full support.
- Expose typed capability statuses and redacted diagnostics through the CLI and Flutter integration.
- Generate Linux build, qualification, and support views from one platform manifest. Add GNOME Wayland x86_64 as an experimental target; keep Ubuntu 24.04 GNOME 46 X11 x86_64 as the stable native qualification target.
- Reduce unnecessary compositor builds and collect comparable CI timing evidence. Preserve exact signed artifact verification, upgrade tests, and native acceptance before publication.

Refresh OCR, Semantic Search, and Supabase packages to 0.1.21 for CopyPaste 1.0.30.

Optional modules remain available from Settings. OCR engines and models are not bundled with the application. Cloud Sync is not part of this release. GNOME Wayland remains experimental and does not have stable native parity evidence.
