# Multi-platform build scripts

One-command builds for all supported platforms. Outputs land in `builds/`
(gitignored), one subdirectory per `<os>-<arch>`.

## Supported platforms

macOS (arm64 + x86_64 + universal) and Android (arm64-v8a + armeabi-v7a).
**Windows is FROZEN as of 2026-05-23** — see
[`docs/adr/ADR-012-windows-frozen-homebrew-only.md`](../../docs/adr/ADR-012-windows-frozen-homebrew-only.md).
The Windows toolchain, container image, and `build-windows.sh` script are
retained as a no-op shim plus reference material for an eventual thaw.

## Quick start

```bash
bash scripts/build-all.sh             # all platforms (missing toolchains skipped)
bash scripts/build-all.sh macos       # macOS only (arm64 + x86_64 + universal)
bash scripts/build-all.sh android     # Android only (arm64-v8a + armeabi-v7a)
```

Individual per-platform scripts:

```bash
bash scripts/build-macos.sh arm64           # or x86_64, universal
bash scripts/build-android-pkg.sh arm64-v8a # or armeabi-v7a, x86_64, x86
```

## Output layout

```
builds/
├── macos-arm64/
│   ├── copypaste-daemon
│   ├── copypaste
│   └── CopyPaste.app/         (when host arch matches)
├── macos-x86_64/
│   ├── copypaste-daemon
│   └── copypaste
├── macos-universal/
│   ├── copypaste-daemon       (lipo'd arm64 + x86_64)
│   └── copypaste
├── android-arm64-v8a/
│   └── libcopypaste_android.so
└── android-armeabi-v7a/
    └── libcopypaste_android.so
```

## Required toolchains

Install once before running `build-all.sh`. Missing toolchains cause individual
platforms to skip rather than abort the whole run.

### Rust targets

```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
```

### Android NDK + cargo-ndk

```bash
cargo install cargo-ndk
# Install Android NDK via Android Studio SDK Manager (or sdkmanager CLI),
# then export:
export ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk/<version>"
```

## Notes per platform

### macOS

- `arm64` and `x86_64` produce native single-arch binaries.
- `universal` requires both `arm64` and `x86_64` to be built first; combines
  them with `lipo -create`.
- `.app` bundling (via `scripts/make_app_bundle.sh`) only runs when the build
  arch matches the host arch (or is `universal`), because the bundler reads
  `target/release/` which is host-tied.

### Android

- This script (`scripts/build-android-pkg.sh`) is for **distribution packaging**:
  copies `.so` files into `builds/android-<abi>/`.
- The pre-existing `scripts/build-android.sh` is for **Gradle integration**:
  writes `.so` to `android/app/src/main/jniLibs/<abi>/` and regenerates UniFFI
  Kotlin bindings. Use that one when developing the Android app.

### Windows (FROZEN)

- `scripts/build-windows.sh` is a no-op shim that exits 0 with a notice.
- `docker/Dockerfile.windows` and the `windows` docker-compose service are
  preserved but inert. No CI job builds them.
- Thaw requires reverting ADR-012.

## Bash compatibility

All scripts are written for **bash 3.2** (macOS default). No `;;&` fallthrough,
no associative arrays. Verify with:

```bash
bash -n scripts/build-all.sh
bash -n scripts/build-macos.sh
bash -n scripts/build-android-pkg.sh
```
