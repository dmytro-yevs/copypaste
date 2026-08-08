# Android API levels — what the spread is supposed to move

**Status:** proposed · 2026-08-08
**Scope:** which API levels the Android layer runs, which assertions are
expected to differ between them, and which pass identically everywhere.
**Related:** [testing-policy.md](testing-policy.md) (the authority on what
counts as verified), [android-clipboard-access.md](android-clipboard-access.md)
(the gates themselves), [android-spike.md](android-spike.md).

Testing one API level tests one point on an API-gated curve. `android-emulator.yml`
runs 36 on its nightly and takes `api-level` as a `workflow_dispatch` input;
`scripts/release/android-smoke-levels.sh` runs the spread locally.

## The four levels, and what each is there for

`minSdk` is 24 and `targetSdk` 36.

| Level | Android | Why it is in the spread |
|---|---|---|
| 29 | 10 | Where background clipboard reads closed. Below API 30, so `ShizukuClipboard.isSupported()` is false and rung 2 must present as unsupported rather than merely unavailable. No `POST_NOTIFICATIONS`, no clipboard-access toast |
| 33 | 13 | `POST_NOTIFICATIONS` becomes a runtime permission. Clipboard auto-clear. The last level on which the deprecated `startActivityAndCollapse(Intent)` overload works |
| 34 | 14 | Foreground service types are enforced, so `specialUse` and `FOREGROUND_SERVICE_SPECIAL_USE` start mattering. `startActivityAndCollapse(Intent)` throws from here |
| 36 | 16 | `targetSdk`, and the only level anything has run on |

## Expected to differ

Each row is a branch that exists in the tree, not a platform note.

| Behaviour | 29 | 33 | 34 | 36 | Where |
|---|---|---|---|---|---|
| `POST_NOTIFICATIONS` is a runtime permission at all | no | yes | yes | yes | `CaptureNotifications.kt:33`, `:61` |
| `adb install -g` grants it | no-op | grants | grants | grants | `android-smoke.sh`, Install |
| Rung 2 is supported by the platform | **no** | yes | yes | yes | `ShizukuClipboard.kt:81` |
| Tile click path | `Intent` overload | `Intent` overload | `PendingIntent` | `PendingIntent` | `CaptureTileService.kt:22` |
| A foreground service's declared type is enforced | no | no | yes | yes | manifest, `CaptureService` |
| Reading another app's clip toasts | no | yes | yes | yes | API 31; `showAccessNotificationLocked` |
| Notification channels are required | yes | yes | yes | yes | `CaptureNotifications.kt:39` |

The last row is the point of listing it: the `SDK_INT >= O` branches in
`CaptureNotifications.builder`, `ensureChannels` and `CaptureService.start` have
`else` arms reachable at 24 and 25, and the lowest level in the spread is 29. No
run at any level executes them. Either the spread gains a level or `minSdk`
rises; leaving both is how dead-on-arrival code ships to the oldest supported
device.

## Assertions that pass identically at every level

An assertion that never touches the gated path is worse than no assertion,
because a green row reads as coverage. These three are in `android-smoke.sh`'s
rung-2 group and every one of them passes at 29 for exactly the reason it passes
at 36 — nothing arms rung 2 on a stock emulator, so no gated path is entered.

| Assertion | Why it cannot move |
|---|---|
| `Shizuku is absent, as it must be on a stock emulator` | A `pm list packages` grep. Identical at every level, and it never asks the app what it reports — which is the one thing that genuinely differs at 29, where `isSupported()` is false |
| `no foreground capture service is running` | `CaptureService` is never started at any level, so the API 34 service-type enforcement is untouched by the run that is supposed to cover it |
| `no notification claims background capture` | Nothing posts one. The assertion cannot separate "correctly silent" from "blocked by a missing `POST_NOTIFICATIONS` grant" from "the code never ran", and those three are different products |

They are worth keeping — each would fail an app that claimed background capture
it did not have, which is rule 4's data-loss argument in its Android form. What
they are not is evidence about a level.

Two more that are level-blind by design and correctly so: the `ACTION_SEND` and
`ACTION_PROCESS_TEXT` doorways read intent extras, not the clipboard, so they
are ungated and must work at all four. A difference there would be a defect.

The Quick Settings tile was the opposite case, and `android-rungs.sh` has since
closed it: one `click-tile` has to change the store or the run fails. That
assertion does branch at 34, so the spread would now notice — except that the
level runner does not run it. `android-rungs.sh` hard-codes AOSP's API 36
argument vector `(String callingPackage, String attributionTag, int userId, int
deviceId)` for its clipboard read, so running the whole script at 29 or 33 would
fail on the vector rather than on the tile. Running the tile group across the
spread means splitting it out first.

## What no level records

`assert_painted` establishes that *a* WebView painted. Nothing records *which*.
`dumpsys webviewupdate` names the package and version in one line, and
`android-smoke-levels.sh` writes it per level for that reason — an emulator can
at least say which build its system image pinned, which is the closest thing
available to the Play-updated WebView that
[testing-policy.md](testing-policy.md) marks NOT VERIFIED IN CI.

## Running it

`scripts/release/android-smoke.sh` is unchanged by this: the runner boots and
selects devices, the harness asserts.

```sh
APK=/path/to/app-debug.apk ./scripts/release/android-smoke-levels.sh 29 33 34 36
```

AVDs are `copypaste-api<level>` (`AVD_PREFIX` overrides), evidence lands under
`artifacts/android-smoke-levels/api<level>/`, and anything other than PASS on
every requested level fails the run — a missing AVD must not read as coverage.

Two things it asserts before it trusts a run:

- **No device may be attached when it starts.** It owns adb for its whole run.
  `android-smoke.sh` makes every adb call unqualified, so a second emulator
  breaks a concurrent run rather than merely confusing this one. The EXIT trap
  that kills the emulator is armed only after this check passes.
- **The booted image must report the API level asked for.** An AVD's name is not
  evidence of its system image, and a spread that silently ran 36 four times
  would be the failure this whole document is about.

## Observed, and not

Taken on 2026-08-08, Windows host, `copypaste-api36`:

- WebView is `com.google.android.webview 133.0.6943.137`, bundled in the system
  image.
- The app installs with `POST_NOTIFICATIONS` `granted=true`, which is `adb
  install -g` doing what the harness's comment says it does — at this level.
- The runner's exclusivity guard refuses while that emulator is attached, and
  leaves it attached.

**Not observed:** no API level other than 36 has been booted, the spread has
never been run end to end, and every "expected to differ" row above is read off
the tree and the platform documentation rather than off a device. None of it is
verification; [testing-policy.md](testing-policy.md) decides that, and it counts
the workflow.
