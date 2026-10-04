# Development

The repository currently contains a Rust core, a local Supabase schema/RLS
harness, and a Flutter application shell. Product screens and the Rust bridge
are not implemented yet.

Run retained backend checks with cargo test --workspace --locked and
./supabase/dev/verify-schema.sh.

When the Flutter foundation is present, set
COPYPASTE_FLUTTER_BUILD_TARGET to macos, apk, or windows and run
scripts/ci/verify-flutter-foundation.sh. It verifies the Flutter SDK and pinned
shadcn_flutter dependency, resolves dependencies, checks formatting and
analysis, runs widget and controller tests, and creates a debug build for the
selected target. A passing build does not qualify a product release.

To run a desktop development build, enter `apps/copypaste_flutter` and run
`flutter run -d macos` or `flutter run -d windows` on the corresponding host.
Press `r` for hot reload or `R` for hot restart. Native code and dependency changes
require stopping and rebuilding the application. Android development uses
`flutter run -d <device-id>` with an explicitly selected device or emulator.

For automatic reload on macOS, run Flutter with a PID file:

```bash
cd apps/copypaste_flutter
flutter run -d macos --pid-file /tmp/copypaste-flutter-dev.pid
```

In another terminal at the repository root, start the source watcher:

```bash
python3 scripts/flutter-hot-reload.py --pid-file /tmp/copypaste-flutter-dev.pid
```

The watcher requests a hot reload for Dart changes, including `main.dart`.
Stop it with Ctrl+C. Startup, desktop-host lifecycle, native code, and
dependency changes may require a full Flutter process restart: send `q` to the
Flutter session and run it again. Do not rely on automatic reload to recreate
native tray or window resources. Windows developers can use their Flutter IDE's
reload-on-save support.

Write all source code, comments, and documentation in English.
