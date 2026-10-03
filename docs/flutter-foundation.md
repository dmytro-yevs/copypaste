# Flutter foundation

The replacement client lives at apps/copypaste_flutter.

- Flutter SDK: .flutter-version
- Product version: root Cargo.toml workspace.package.version
- UI dependency: shadcn_ui: 0.57.1
- Android targets: ARM32, ARM64 and x64; x86 is retired

This milestone configures the toolchain only. It does not add screens, product
widgets, a Rust bridge, platform hosts, updater behavior or release packaging.
release.yml fails intentionally until those obligations are implemented and qualified.
The foundation check requires COPYPASTE_FLUTTER_BUILD_TARGET set to macos, apk,
or windows.
