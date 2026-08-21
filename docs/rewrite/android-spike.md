# The Android device spike

**Status:** open · rung 0 runs on an emulator; rung 2's platform half is now
observed, its Shizuku half is not
**No longer blocking:** [android-clipboard-access.md](android-clipboard-access.md)
§4's claim — a binder call as the shell uid with
`callingPackage = "com.android.shell"` reads the clipboard with no focus — was
derived from AOSP source and has now been run. The API 36 emulator leg runs
`scripts/release/android-rungs.sh`.

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

* **The UI paints, and it is now asserted.** The React side reaches the
  WebView: 33 named accessibility nodes under it, the first reading
  "CopyPaste". It arrives late — 33 and 38 seconds after `am start` across two
  runs — which is why run 2, sampling once at 25, found an empty WebView and
  passed anyway. The check polls to a 90-second budget and reports the time it
  actually took, so the margin stays measured.

### What the second script settled

`scripts/release/android-rungs.sh` runs after the smoke test on the same booted
device. On API 36, x86_64, `google_apis`:

* **The shell uid reads the clipboard with no focus.** `service call clipboard`
  as `com.android.shell` returns a clip another app copied, while that app has
  focus and ours does not. The same call naming our own package is refused with
  `Package com.copypaste.app does not belong to 2000` from
  `AppOpsManager.checkPackage` — so item 5 below passes for shell and the
  identity, not the transport, is what grants the read. AOSP's argument vector
  `(String callingPackage, String attributionTag, int userId, int deviceId)`
  is this API level's, which settles item 4.
* **The Quick Settings tile works end to end.** One `click-tile` starts
  `ClipboardCaptureActivity` and the clip reaches SQLCipher unreadable.
* **`FLAG_SECURE` is on the window from the first dump onwards**, twenty dumps
  over a minute, with another window on the same device reported unprotected by
  the same reader.
* **Nothing claims to capture when nothing is listening.** With `enabled=true`
  written straight into `shared_prefs/capture-service.xml`, no foreground
  service runs and no notification appears.

Two things that were reported as dead ends and are not:

* **`cmd statusbar add-tile` and `click-tile` work.** They print nothing either
  way. `add-tile` is visible in `sysui_qs_tiles`, and `click-tile` reaches a
  third-party tile *once SystemUI has bound it* — which it does lazily, so the
  panel has to be opened once first. Clicking without that is a silent no-op,
  which is what read as "prints nothing".
* **Text can be put on the clipboard without `cmd clipboard`.** It still does
  not exist. Driving another app's text field does: Settings' search box,
  `input keycombination` for select-all, `KEYCODE_COPY` — and
  `getPrimaryClipSource` then names that app, so the clip is genuinely foreign.

What is still **not** settled, and why:

* **The Shizuku setup bridge.** Pairing, one-shot grant application and the
  optional toast-setting write still need a phone with Shizuku paired by hand.
* **R8.** The APK is the debug one because every filesystem assertion goes
  through `run-as`, so the minified release build's plugin reflection is
  untested.
* **The loss notification after a reboot.** The binder death recipient that
  posts it lives in the process that died, so a cold start with the armed flag
  still on disk posts nothing. Recorded as a probe rather than a failure: §5
  rule 3 wants loss pushed, and whether the app should re-post on start from
  the persisted flag is a decision, not a defect.

Everything unobservable is printed under `NOT ASSERTED` rather than skipped, and
`check.sh` runs both scripts' `--self-test` so their detectors are known to fail
when they should.

### Two traps, each of which cost a run

`extractNativeLibs` is `false` — AGP's default at minSdk 24 — so the library is
mapped straight out of `base.apk` and the string `libcopypaste_ui_lib.so`
appears nowhere in `/proc/<pid>/maps`. Asserting that file name failed while the
library was demonstrably running. The evidence is an *executable* mapping owned
by the package (`r-xp … /base.apk`), and anything else reading maps should
expect the same.

`screencap` is not evidence here. Under `-gpu swiftshader_indirect -no-window`
it returned the same 2 KB image for a fully painted UI as for a blank one. The
accessibility tree is the signal; the screenshot is for a human to look at.

## On a phone, not an emulator

Each of these would falsify something currently written as true. Items 4 and 5
moved on an emulator; what is left is the Shizuku transport and the hardware.

1. ~~**The Kotlin does not compile.**~~ Settled: it builds, installs, and runs.
   The AIDL package placement is still only known to *build* — whether a system
   binder accepts the stub's interface descriptor is item 3's question.
2. **`Shizuku.newProcess` is not reachable by reflection** in the version of
   the API library resolved. Only the toast-suppression opt-in depends on it;
   everything else fails independently.
3. **The app-owned ClipCascade path still needs real device proof.** The
   runtime path is `READ_LOGS` + overlay focus + foreground service; what is
   missing is proof on actual OEM builds, not another Shizuku-side fallback.
4. ~~**The argument vector is wrong on this API level.**~~ Settled on API 36:
   `(String callingPackage, String attributionTag, int userId, int deviceId)`
   is the order, which is what `invoke` builds. Still open on other API levels,
   and `lastFailure` records it if it moves.
5. ~~**`mAppOps.checkPackage(2000, "com.android.shell")` refuses.**~~ Settled on
   API 36: it passes for the shell package and refuses every other, which is
   the whole hinge. Still open on an OEM build.
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
  without the Shizuku user service? `ShizukuClipboard.isToastSuppressed`
  assumes it is.
