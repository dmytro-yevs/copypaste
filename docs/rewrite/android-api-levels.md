# Android API-level matrix

**Status:** maintained · 2026-08-08
**Scope:** which Android platform boundary each emulator level represents, and
what evidence a green leg does and does not establish.
**Authority:** [testing-policy.md](testing-policy.md) defines when a result is
verified. [android-clipboard-access.md](android-clipboard-access.md) defines the
capture ladder.

One emulator level is one point on an API-gated curve. The scheduled workflow
includes API 24, 29, 33, 34, and 36; a dispatch selects one level. API 24 checks
the supported runtime floor. The four behavior boundaries recovered by this
matrix are API 29, 33, 34, and 36.

## Why each level exists

`minSdk` is 24 and `targetSdk` is 36.

| API | Android | Boundary represented |
|---|---|---|
| 24 | 7.0 | Oldest supported runtime; reaches the pre-notification-channel branches |
| 29 | 10 | Background clipboard reads close to unfocused apps; on-device Shizuku setup remains unsupported below API 30 |
| 33 | 13 | `POST_NOTIFICATIONS` becomes a runtime permission; clipboard auto-clear applies |
| 34 | 14 | Foreground-service types are enforced; Quick Settings tile launch switches to the `PendingIntent` overload |
| 36 | 16 | Current target; the API-specific shell clipboard transaction used by `android-rungs.sh` is asserted here |

These are representative boundaries, not a claim that intermediate versions
cannot differ. API 31 introduced clipboard access notices, for example, but API
33 is the selected post-notice level because it also crosses the notification
permission and auto-clear boundaries.

## Branches expected to move

| Behavior | 24 | 29 | 33 | 34 | 36 | Implementation |
|---|---|---|---|---|---|---|
| Notification channels required | no | yes | yes | yes | yes | `CaptureNotifications` |
| `POST_NOTIFICATIONS` runtime permission | no | no | yes | yes | yes | `CaptureNotifications` |
| Rung 2 offered for on-device setup | no | no | yes | yes | yes | `ShizukuClipboard.isSupported` |
| Tile launch overload | `Intent` | `Intent` | `Intent` | `PendingIntent` | `PendingIntent` | `CaptureTileService` |
| Declared foreground-service type enforced | no | no | no | yes | yes | `AndroidManifest.xml` / `CaptureService` |
| Clipboard access notice available | no | no | yes | yes | yes | Android clipboard service |

The workflow's ordinary smoke and storage-transfer legs run at every selected
level. They establish launch, paint, Keystore/database continuity, the two text
intake intents, and document export/import at that level. Their negative
Shizuku assertions remain valuable, but they do not exercise a platform branch:
a stock emulator never arms rung 2.

`android-rungs.sh` stays on API 36. It positively asserts the shell-UID read,
Quick Settings capture, the negative foreground-service state, and
`FLAG_SECURE`; its raw `service call clipboard` argument vector is API-specific.
Moving only its tile assertion into the wider matrix would require an explicit
seam in that harness, not another copy of its capture logic.

Two-device scenarios likewise remain with their serialized hardware validation
owner. An API-level smoke loop is neither concurrency-safe evidence for them nor
a replacement harness.

## Local diagnostic runner

`android-smoke-levels.sh` runs the existing debug smoke harness against named
AVDs, one at a time. Its default is the four behavior boundaries; pass API 24
explicitly when diagnosing the runtime floor.

```sh
APK=/path/to/app-debug.apk ./scripts/release/android-smoke-levels.sh 29 33 34 36
```

AVDs are named `copypaste-api<level>` by default. Evidence is separated under
`artifacts/android-smoke-levels/api<level>/`, including the system-image
fingerprint and pinned WebView package. A missing AVD, wrong system image, or
failed smoke leg fails the overall run.

The runner refuses to start while any adb device is attached. Native Android
execution is serialized elsewhere, so recovery and static-check work must use
`--self-test`, syntax checks, and workflow wiring checks instead of borrowing or
resetting an owned emulator.

The system image's WebView record is diagnostic evidence only. A user's WebView
updates from Play independently of the OS, so the emulator cannot verify that
shipped configuration.
