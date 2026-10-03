# CopyPaste

CopyPaste is being rebuilt as a Flutter application with a Rust core.

The repository is at a foundation-only milestone. The React/Tauri UI, browser
harnesses, WebView checks and application release pipeline are gone. No product
screens, Rust bridge, platform hosts, updater or release artifacts exist yet.

Rust and the local Supabase schema/RLS harness remain active. The Flutter
foundation uses .flutter-version, and apps/copypaste_flutter/pubspec.yaml must
match Cargo.toml workspace.package.version.

See [Flutter foundation](docs/flutter-foundation.md).
