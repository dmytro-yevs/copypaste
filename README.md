# CopyPaste

CopyPaste combines a shared Flutter application with a Rust runtime for encrypted
clipboard history, paired-device synchronization, search, and Quick Paste.
Desktop integration and mobile capture sit behind typed platform adapters.

The product targets macOS, Android, Windows, and Linux. Linux's
stable release contract covers x86_64 GNOME 46 on X11 (Ubuntu 24.04), with
AppImage and Debian packages. ARM64, KDE, Wayland, and RPM implementation remains
available for development through opt-in experimental validation. See the [Linux platform contract](docs/linux-platform.md)
for required integrations and the explicit screenshot-protection exception.
The declared targets do not establish that a release has passed native checks.

Flutter is pinned in `.flutter-version`. The version in
`apps/copypaste_flutter/pubspec.yaml` must match `Cargo.toml`'s workspace version.
Production publication requires exact-artifact qualification and signatures;
development builds and unit tests are separate from that evidence.

See [development](docs/development.md), [Flutter architecture](docs/flutter-foundation.md),
and [release qualification](docs/release-qualification.md).
