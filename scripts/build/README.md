# Multi-platform build scripts

One-command builds for all supported platforms. Outputs land in `builds/`
(gitignored), one subdirectory per `<os>-<arch>`.

## Quick start

```bash
bash scripts/build-all.sh             # all platforms (missing toolchains skipped)
bash scripts/build-all.sh macos       # macOS only (arm64 + x86_64 + universal)
bash scripts/build-all.sh android     # Android only (arm64-v8a + armeabi-v7a)
bash scripts/build-all.sh windows     # Windows x86_64 (best-effort)
```

Individual per-platform scripts:

```bash
bash scripts/build-macos.sh arm64           # or x86_64, universal
bash scripts/build-android-pkg.sh arm64-v8a # or armeabi-v7a, x86_64, x86
bash scripts/build-windows.sh x86_64
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
├── android-armeabi-v7a/
│   └── libcopypaste_android.so
└── windows-x86_64/
    └── copypaste-daemon.exe   (best-effort)
```

## Required toolchains

Install once before running `build-all.sh`. Missing toolchains cause individual
platforms to skip rather than abort the whole run.

### Rust targets

```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
rustup target add x86_64-pc-windows-gnu
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

### Windows cross-compile (mingw-w64)

```bash
brew install mingw-w64    # macOS
# OR
sudo apt install mingw-w64  # Linux
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

### Windows

- Cross-compile via mingw-w64 is **best-effort**. The daemon's IPC layer
  currently has only a stub for Windows; link errors are expected.
- For production Windows builds, use a real Windows host or MSVC toolchain.
- When the build fails it is reported as `SKIP` in the summary, not a hard
  error — so `build-all.sh` finishes for other platforms.

## Bash compatibility

All scripts are written for **bash 3.2** (macOS default). No `;;&` fallthrough,
no associative arrays. Verify with:

```bash
bash -n scripts/build-all.sh
bash -n scripts/build-macos.sh
bash -n scripts/build-android-pkg.sh
bash -n scripts/build-windows.sh
```
