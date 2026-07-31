# The Android device spike

**Status:** open · the APK builds; nothing in it has been observed running
**Blocking:** the rung 2 recommendation in
[android-clipboard-access.md](android-clipboard-access.md) §4 rests on a claim
derived from AOSP source and never observed.

The Kotlin compiles: the release workflow's `android` job produced a signed
28 MB universal APK. That is the whole of what is known. No line of Kotlin, of
`crypto/keystore/android.rs` or of `capture/android.rs` has executed.

## Before the phone

1. `.github/workflows/android-emulator.yml` — dispatch it. It builds a
   debuggable x86_64 APK and runs `scripts/release/android-smoke.sh` against a
   booted AVD. Half the list below is answerable there, without a device.
2. On a workstation: SDK platform 36 with build-tools, NDK r27, then
   `cargo tauri android build --debug --apk`. The APK lands under
   `gen/android/app/build/outputs/apk/`.

## What the emulator answers, and what it cannot

Item 1 is settled — the Kotlin compiles. Of the rest, the emulator reaches only
what needs no Shizuku: that the activity starts and `libcopypaste_ui_lib.so`
loads, that a second launch reads back the device secret the first one minted
(the one observation ADR-0003 asks for), that the share-sheet and
text-selection doorways reach SQLCipher, and that the database is not a
readable SQLite file.

Items 2 to 7 stay open and stay open on purpose. Shizuku needs a
wireless-debugging pairing performed by hand, so rung 2 cannot be granted on a
stock emulator and faking it would prove nothing. What the job asserts instead
is the honest consequence: with rung 2 unavailable, no foreground service runs
and no notification claims background capture. Item 6 — an OEM battery manager
killing the service — is not reproducible on an emulator at all.

The job prints everything it could not observe under `NOT ASSERTED` rather than
skipping it, and `scripts/release/check.sh` runs the smoke test's `--self-test`
so its detectors are known to fail when they should.

## On the phone, in likelihood order

Each of these would falsify something currently written as true.

1. ~~**The Kotlin does not compile.**~~ Settled: the release job's APK
   contains it. The AIDL package placement is still only known to *build* —
   whether a system binder accepts the stub's interface descriptor is item 3's
   question.
2. **`Shizuku.newProcess` is not reachable by reflection** in the version of
   the API library resolved. Only the toast-suppression opt-in depends on it;
   everything else fails independently.
3. **`addPrimaryClipChangedListener` registers and never fires.** The fallback
   is already written and documented: poll `getPrimaryClip` over the same proxy
   on a timer. `ShizukuClipboard.pollOnce` is that call; what is missing is the
   timer, and the state model needs no change because a polled read reports the
   same `ReadOutcome`.
4. **The argument vector is wrong on this API level.** `invoke` assumes AOSP's
   ordering — first `String` is the calling package, first `int` is the user id.
   A `SecurityException` naming a uid, or a `NullPointerException` inside
   `system_server`, is this. `lastFailure` records it.
5. **`mAppOps.checkPackage(2000, "com.android.shell")` refuses.** This is the
   one that would falsify the whole rung. It would appear as a
   `SecurityException` on the first `getPrimaryClip`, not as a null. If it
   happens, rung 2 is not available on this device and the honest answer is the
   state model's `ReadRefused`, which is already wired to say so and to offer no
   button that would not help.
6. **The foreground service is killed anyway** by an OEM battery manager
   (Xiaomi, Samsung). Capture stops without the binder dying, so nothing
   notices: the `Working` state would persist while nothing is captured. **This
   is the failure mode the design is weakest against** — check it explicitly by
   leaving the phone idle for an hour and then copying in another app.
7. **The Android 12+ toast is per-copy rather than per-(uid, clip)** in
   practice, making rung 2 unusable without the suppression opt-in. Observe how
   often "Shell pasted from your clipboard" actually appears.
8. **Wireless-debugging pairing fails on this OEM build.** Shizuku's own FAQ
   lists MIUI-specific failures; it costs the user rung 2 and nothing else.

## What to answer while you are there

* Does the state ever reach `Working`? It requires a read taken **without**
  focus, so: arm, leave the app, copy something in another app, come back.
* Does the loss notification arrive on reboot, and does tapping it land on the
  rung 2 screen with the start step selected?
* Does the tile save the clipboard in one tap, and how visible is
  `IntakeActivity` while it does?
* Is `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS` readable by the app
  without the shell uid? `ShizukuClipboard.isToastSuppressed` assumes it is.
