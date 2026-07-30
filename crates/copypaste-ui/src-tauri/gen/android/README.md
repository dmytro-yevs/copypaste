# gen/android

Generated once by `cargo tauri android init` and then hand-edited. Tracked in
git, because regenerating it in CI would not be deterministic and because the
files below no longer match the template.

**Re-running `tauri android init` overwrites these. Diff before you accept it.**

| File | What was added |
|---|---|
| `app/src/main/AndroidManifest.xml` | `IntakeActivity`, `CaptureTileService`, `CaptureService`, Shizuku's provider, and the permissions the capture ladder needs |
| `app/build.gradle.kts` | the Shizuku client library, and `aidl = true` |
| `app/src/main/res/values*/themes.xml` | `Theme.CopyPaste.Invisible` |
| `app/src/main/res/values/strings.xml` | `capture_action` |
| `app/src/main/java/com/copypaste/app/MainActivity.kt` | `onNewIntent`, so the loss notification's extra is not read off a stale intent |

Everything else under `app/src/main/java/com/copypaste/app/` and the one
`.aidl` is ours and was never in the template.

The Kotlin holds no policy — see
[ADR-0005](../../../../docs/adr/0005-android-capture-in-rust-kotlin-reports.md).
It has never been compiled: this project's host has no Android SDK. The first
build will find things, and
[docs/rewrite/android-spike.md](../../../../docs/rewrite/android-spike.md) lists
them in the order to expect.
