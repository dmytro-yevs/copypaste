Improve clipboard file handling, History, and desktop Quick Paste in CopyPaste 1.0.35.

- Keep newly captured files as references to their original paths or Android content URIs, and read file contents only for an explicit transfer. Existing retained snapshots remain readable; new references require the original file and provider access to remain available.
- Move copied items to the top of History while preserving their identity and pin state, and keep rebuilt History pages stable while scrolling.
- Improve Quick Paste activation, focus restoration, repeated opening, and hidden-session cleanup on desktop platforms.
- Improve Windows clipboard file paths and image format handling, global clipboard shortcuts, onboarding, and application updates.
- Avoid Android keyboard flicker during clipboard capture and improve file-reference metadata handling.
- Improve desktop clipboard source classification, notifications, text layout, and the empty state for modules without configurable settings.
- Keep paired-device group membership consistent across connected devices.
- Update ctutils and Tokio to resolve the RustSec constant-time and I/O-safety advisories.
- Bind OCR, Semantic Search, and Supabase module packages 0.1.26 to CopyPaste 1.0.35.

Paired-device synchronization uses protocol v10. Update paired devices to compatible versions together; protocol v9 peers cannot establish a session with this release. Optional modules remain available from Settings; OCR engines and models are not bundled with the application. Cloud Sync is not part of this release.

Linux retains the Ubuntu 24.04 GNOME 46 X11 x86_64 release scope. RPM, ARM64, KDE, Wayland, and full RPM desktop parity retain their existing qualification limits. Native and resource qualification is determined by the release evidence. Native resource evidence and resolved budgets remain required before claiming resource qualification.
