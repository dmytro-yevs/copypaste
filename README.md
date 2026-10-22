# CopyPaste

CopyPaste combines a shared Flutter application with a Rust runtime for encrypted
clipboard history, paired-device synchronization, search, and Quick Paste.
Desktop integration and mobile capture sit behind typed platform adapters.

The product targets macOS, Android, Windows, and Linux. Linux's
stable release contract covers x86_64 GNOME 46 on X11 (Ubuntu 24.04), with
AppImage and Debian packages. ARM64, KDE, and Wayland implementations remain
available for development through opt-in experimental validation. Screenshot
protection remains an explicit Linux release limitation.
Releases also provide a signed experimental x86_64 RPM with Fedora 44 package
installation evidence. RPM desktop parity is not yet qualified.
The declared targets do not establish that a release has passed native checks.

| Distribution | Desktop | Session | Architecture | Packages | Tier | Native qualification |
| --- | --- | --- | --- | --- | --- | --- |
| ubuntu 24.04 | GNOME 46 | x11 | x86_64 | AppImage, deb | stable | required |
| ubuntu 24.04 | GNOME 46 | wayland | x86_64 | AppImage, deb | experimental | not yet qualified |
| ubuntu 24.04 | GNOME 46 | wayland | aarch64 | AppImage, deb | experimental | not yet qualified |
| fedora 44 | KDE 6 | wayland | x86_64 | rpm | experimental | not yet qualified |

Flutter is pinned in `.flutter-version`. The version in
`apps/copypaste_flutter/pubspec.yaml` must match `Cargo.toml`'s workspace version.
Production publication requires exact-artifact qualification and signatures;
development builds and unit tests are separate from that evidence.

The [production workflow](.github/workflows/release.yml) owns the build,
qualification, signing, and publication sequence.
