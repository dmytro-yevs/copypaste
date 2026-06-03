# Android release signing

The Release workflow (`.github/workflows/release.yml`, `build-android` job) builds a
**release-signed** APK via `./gradlew assembleRelease` when the signing secrets are
present. When they are absent (forks, local builds) the build falls back to a
**debug-signed** release APK so it never hard-fails — see the `release` signingConfig
fallback in `android/app/build.gradle.kts`.

## Required GitHub repository secrets

All four must be set for a release-signed APK. If any is missing, the workflow logs a
warning and produces a debug-signed APK instead.

| Secret | Meaning |
|--------|---------|
| `ANDROID_KEYSTORE_BASE64` | base64 of the release keystore (`.jks` / `.keystore`) |
| `ANDROID_KEYSTORE_PASSWORD` | keystore (store) password |
| `ANDROID_KEY_ALIAS` | signing key alias |
| `ANDROID_KEY_PASSWORD` | key password |

CI decodes `ANDROID_KEYSTORE_BASE64` to `$RUNNER_TEMP/release.keystore`, then passes
the path + credentials to Gradle as project properties
(`-PANDROID_KEYSTORE_FILE`, `-PANDROID_KEYSTORE_PASSWORD`, `-PANDROID_KEY_ALIAS`,
`-PANDROID_KEY_PASSWORD`). `build.gradle.kts` reads these (falling back to the
matching environment variables for local use) and creates `signingConfigs.release`
only when the keystore file actually exists. Secret values are never echoed.

## Generating the keystore

```bash
keytool -genkeypair -v \
  -keystore release.keystore \
  -alias copypaste \
  -keyalg RSA -keysize 2048 -validity 10000
# then base64 it for the secret:
base64 -i release.keystore | pbcopy   # macOS; on Linux: base64 -w0 release.keystore
```

Set the four secrets in the repo: **Settings → Secrets and variables → Actions**.

Do **not** commit a real release keystore. The committed `android/app/debug.keystore`
is the standard, non-secret debug key used only for the fallback path.

## Versioning (tag-authoritative)

`versionName` / `versionCode` are derived from the pushed git tag in CI (mirrors the
macOS/tauri job), passed as `-PversionName` / `-PversionCode`, and read by
`build.gradle.kts` with dev defaults. `versionCode = MAJOR*10000 + MINOR*100 + PATCH`
(e.g. `v0.6.0` → name `0.6.0`, code `600`) so a tagged release is always monotonic
regardless of the hardcoded dev defaults.
