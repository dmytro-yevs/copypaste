# The Android device spike

**Status:** open · everything below is untested code
**Blocking:** the rung 2 recommendation in
[android-clipboard-access.md](android-clipboard-access.md) §4 rests on a claim
derived from AOSP source and never observed.

Nothing in `crates/copypaste-ui/src-tauri/gen/android/` has been compiled. This
host has no Android SDK, `dl.google.com` is unreachable from it, and
`cargo check --target aarch64-linux-android` stops at SQLCipher for want of an
NDK `clang`. So the first run on a real machine will find several things at
once, and this is the order to expect them in.

## Before the phone

1. Install the Android SDK (platform 36, build-tools) and NDK r26+, then
   `cargo tauri android build --debug --apk`.
   The APK lands under `gen/android/app/build/outputs/apk/`.
2. Expect the first failures to be build failures, not behaviour: the Gradle
   project was generated against a stubbed SDK path and its Kotlin has never
   been through a compiler.

## On the phone, in likelihood order

Each of these would falsify something currently written as true.

1. **The Kotlin does not compile.** Most likely in `ShizukuClipboard`'s
   reflection or in the `IOnPrimaryClipChangedListener` AIDL package placement
   (`android.content` must match AOSP's, since the stub is passed to a system
   binder that checks the interface descriptor).
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
