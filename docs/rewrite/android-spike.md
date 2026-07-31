# The Android device spike

**Status:** open · rung 0 runs on an emulator; rung 2 is still only read, not
observed
**Blocking:** the rung 2 recommendation in
[android-clipboard-access.md](android-clipboard-access.md) §4 rests on a claim
derived from AOSP source and never observed.

## What the emulator settled

`.github/workflows/android-emulator.yml` builds a debuggable x86_64 APK and
runs `scripts/release/android-smoke.sh` against a booted API 36 AVD. Two runs
in, on Android 16, x86_64, `google_apis`:

* The app launches. `System.loadLibrary("copypaste_ui_lib")` resolves, the
  activity reaches focus, and a WebView is instantiated in-process.
* **ADR-0003's keystore round-trip holds.** First launch mints and writes
  `shared_prefs/copypaste-device-secret.xml`; after `force-stop` the second
  launch reopens the same database — same page-1 salt — with the blob
  byte-identical. A re-minted secret could not have opened that file.
* SQLCipher over vendored OpenSSL works on Android. `copypaste-v2.db` is not a
  readable SQLite file, and a captured canary is nowhere in it or anywhere else
  under the app's data directory.
* Both rung 0 doorways work end to end: `ACTION_SEND` and `ACTION_PROCESS_TEXT`
  reach `ClipQueue`, the intake drain and `ingest`, and the store changes.
* With Shizuku absent, nothing claims otherwise — no foreground service, no
  ongoing notification.

What it did **not** settle, and why:

* **Whether the UI paints.** Run 1's hierarchy dump held the rendered tree — six
  buttons, four text views — and a 36 KB screenshot. Run 2, same code, held a
  bare `android.webkit.WebView` node and a 2 KB screenshot: at 25 seconds the
  WebView had not painted. So a bare assertion here would be flaky; it needs to
  wait for content, and until it does this stays a probe with both artefacts
  attached.
* **The Quick Settings tile.** `cmd statusbar add-tile` and `click-tile` print
  nothing on this image and `cmd clipboard` does not exist on it — there is no
  way to put text on the clipboard from the shell, so the tile's read has
  nothing to read. Unproven, and the job says so.
* **Rung 2 itself, and R8.** Shizuku needs a pairing done by hand. The APK is
  the debug one because every filesystem assertion goes through `run-as`, so
  the minified release build's plugin reflection is untested.

Everything unobservable is printed under `NOT ASSERTED` rather than skipped, and
`check.sh` runs the smoke test's `--self-test` so its detectors are known to
fail when they should.

### One trap this cost a run

`extractNativeLibs` is `false` — AGP's default at minSdk 24 — so the library is
mapped straight out of `base.apk` and the string `libcopypaste_ui_lib.so`
appears nowhere in `/proc/<pid>/maps`. Asserting that file name failed while the
library was demonstrably running. The evidence is an *executable* mapping owned
by the package (`r-xp … /base.apk`), and anything else reading maps should
expect the same.

## On a phone, not an emulator

Each of these would falsify something currently written as true. Items 2 to 8
are all rung 2 or hardware, which is why none of them moved.

1. ~~**The Kotlin does not compile.**~~ Settled: it builds, installs, and runs.
   The AIDL package placement is still only known to *build* — whether a system
   binder accepts the stub's interface descriptor is item 3's question.
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
