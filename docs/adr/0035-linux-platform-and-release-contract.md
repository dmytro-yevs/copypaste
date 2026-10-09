# ADR-0035 — Linux platform and release contract

**Status:** accepted · 2026-10-09

## Decision

Linux is a product target alongside macOS, Android, and Windows, using the same
Flutter UI and Rust runtime. Support covers x86_64 and ARM64, with AppImage,
Debian, and RPM packages in each regular stable release.

Functional and UI parity is required on X11 and native Wayland on GNOME and KDE.
Wayland Quick Paste includes user-enabled GNOME Shell/KWin companions to restore
the previously focused application and user-consented Remote Desktop keyboard
access. Packages cannot silently enable desktop integrations or approve portals.

The sole explicitly accepted exception is **Block screenshots**. Linux must
show that the option is unavailable and must not report successful protection.
Other privacy controls and encrypted storage retain their existing contract.

## Consequences

Linux uses the shared Unix IPC transport. Production keys belong in Secret
Service, with no plaintext fallback or silent identity replacement. Clipboard
monitoring must use real X11/Wayland adapters, bounded typed payloads, source
exclusion policy, producer privacy hints, and self-write suppression.

Updater selection must match both the running architecture and installation
format. Publisher signatures and artifact digests remain mandatory. System
package installation preserves the package manager's authentication boundary;
AppImage replacement is limited to the existing user-owned executable.

The release gate must bind each Linux package to native evidence and refuse
publication or recovery when required receipts or signatures are missing.
Compilation, portable tests, and one desktop/session cannot qualify a different
desktop/session. The actual qualified support matrix must accompany a release.

See [the Linux acceptance contract](../linux-platform.md). This decision defines
the required behavior; it does not certify an untested or unpublished artifact.
