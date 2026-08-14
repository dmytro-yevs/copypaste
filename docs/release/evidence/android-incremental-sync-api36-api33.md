# Android incremental sync evidence

Date: 2026-08-09

Outcome: the API 36 creator and API 33 joiner each passed the two-device
instrumentation scenario. The test orders the creator's later local write
before the joiner's live write, then requires both pre-pair canaries and both
live canaries on both devices. This covers the lower-clock cursor regression
fixed by `da69777a`.

## Observations

- Both instrumentation results ended in `OK (1 test)` with no failure marker.
- The creator served three sessions; the joiner paired once and completed two
  incremental syncs.
- Both cursor files converged to `since_ms: 1786276185477` with a null relay
  floor.
- The APK and test APK passed `apksigner verify --print-certs` with the same
  debug certificate SHA-256 digest:
  `291c38c88988f281dabb9bc0410b6a15f96f25c25363b16b20d2dec3b7dd6f72`.
- Package state on both devices reported min SDK 24, target SDK 36, and APK
  signing version 2.
- The retained text evidence contained no pairing URI, pairing argument name,
  or host-user path. Product app logs and window metadata contained none of
  the four canaries. The instrumentation assertions also checked logcat and
  window metadata after settled history reads.

The raw run is intentionally ignored build output. Its hashes, plus the APK
and owned-source hashes, are retained in the adjacent checksum manifest.

## Commands

```text
gradlew.bat assembleX86_64DebugAndroidTest -x rustBuildX86_64Debug --no-daemon
bash -n scripts/release/android-two-device-e2e.sh
bash scripts/release/android-smoke.sh --self-test
bash scripts/release/android-smoke-release.sh --self-test
APK=<app> TEST_APK=<test-app> FIRST_DEVICE=emulator-5554 \
  SECOND_DEVICE=emulator-5556 \
  ANDROID_TWO_DEVICE_OUT=<run-output> \
  bash scripts/release/android-two-device-e2e.sh
apksigner verify --print-certs <app>
apksigner verify --print-certs <test-app>
```

The Android test APK compile passed in the verified source worktree whose two
owned Kotlin sources byte-match this commit. On exact current main, Gradle
reached app Kotlin compilation and then failed in the pre-existing
`ShizukuSettings.kt` call to the private `Shizuku.newProcess` API; this is
outside the harness change and is not represented as a pass.

API 24, 29, 31, and 34 device executions remain outstanding.
