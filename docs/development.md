# Development

The repository contains the Rust runtime and a shared Flutter product for
macOS, Android, Windows, and Linux. History, devices, settings, onboarding, native
capture, Quick Paste, and application updates use typed platform boundaries.

Run retained backend checks with cargo test --workspace --locked and
./supabase/dev/verify-schema.sh.

For the development gate, set
COPYPASTE_FLUTTER_BUILD_TARGET to macos, apk, windows, or linux and run
scripts/ci/verify-flutter-foundation.sh. It verifies the Flutter SDK and pinned
shadcn_flutter dependency, resolves dependencies, checks formatting and
analysis, runs widget and controller tests, and creates a debug build for the
selected target. A passing development build does not qualify a product
release.

Production artifacts are owned by `.github/workflows/release.yml`. A manual
run qualifies signed Release artifacts without publishing. See
`docs/release-qualification.md` for the exact-artifact and Keychain rules.

To run a desktop development build, enter `apps/copypaste_flutter` and run
`flutter run -d macos`, `flutter run -d windows`, or `flutter run -d linux` on the corresponding host.
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

Git build-cache cleanup is enabled with `git config core.hooksPath .githooks`.
After a commit and before a push, hooks run `cargo clean` for existing local
workspace and module targets and `flutter clean` for the Flutter app. Cleanup
skips active builds and never fails the Git operation. It discards build
caches, so the next build recompiles. See `docs/adr/0026-bound-the-primary-checkout-target-directory.md`.
