# Development

The repository currently contains a Rust core, a local Supabase schema/RLS
harness, and a Flutter foundation. It does not contain a product UI, platform
host or Rust bridge.

Run retained backend checks with cargo test --workspace --locked and
./supabase/dev/verify-schema.sh.

When the Flutter foundation is present, set
COPYPASTE_FLUTTER_BUILD_TARGET to macos, apk, or windows and run
scripts/ci/verify-flutter-foundation.sh. It resolves dependencies, checks
formatting and analysis, and creates a debug macOS foundation build. It does
not run a zero-test suite or qualify a product release.
