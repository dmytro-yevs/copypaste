# Android clipboard access — what the platform allows, and which rung we ship first

**Status:** current contract · 2026-07-30
**Scope:** how CopyPaste for Android captures clipboard content, what we ask the
user to do for it, and what we tell them when it stops working.
**Related:** [ADR-0001](../adr/0001-macos-distribution-without-a-developer-id.md)
(the "build a product that needs no permission" principle), [ADR-0002](../adr/0002-one-cross-platform-app.md)
(one Tauri v2 app, so all of this lives in an Android plugin, not in shared Rust).

## Decision

Ship a **four-rung ladder**, and present rung 0 first.

1. **Rung 0 is the default and the fallback.** No permission, no setup: in-app
   capture, a share-sheet/text-selection target, a one-tap Quick Settings tile,
   and the Mac's history over sync. A new user who does not know what ADB is
   never has to.
2. **Rung 2 is optional advanced capture.** Shizuku runs the narrowly filtered
   logcat reader as shell or root; CopyPaste receives only an occurrence signal,
   then owns the focused overlay read and reports working only after a real read
   (`CopyPaste-qzhu`).
3. **Direct shell-UID clipboard access is a platform spike, not a second
   shipped reader.** The current implementation stays fail-closed where OEM or
   Android policy prevents its granted runtime path.
4. **We do not build an AccessibilityService.** Contrary to a decade of folklore,
   it is *not* a clipboard exemption in AOSP.
   It would cost the user a scary permission and us a Play declaration, and it
   would not work.

## 1. What the platform actually permits (2026)

The public rule has not changed since Android 10: *"Unless your app is the
default input method editor (IME) or is the app that currently has focus, your
app cannot access clipboard data on Android 10 or higher."*
([Android 10 privacy changes](https://developer.android.com/about/versions/10/privacy/changes))

The enforcement point is `ClipboardService.clipboardAccessAllowed()`. Read
access to the primary clip is granted only if the caller:

| # | Condition | Reachable by us? |
|---|---|---|
| 1 | holds `READ_CLIPBOARD_IN_BACKGROUND` | **only via the shell UID** — see below |
| 2 | is the **default IME** | yes, at the cost of being the user's keyboard |
| 3 | **has window focus** (`mWm.isUidFocused`) | yes, momentarily |
| 4 | holds `INTERNAL_SYSTEM_WINDOW` **and** has focus | no (`signature\|module\|recents`) |
| 5 | is the ContentCapture service | no |
| 6 | is the Augmented Autofill service | no |
| 7 | owns the VirtualDevice being read | no |

…and then the `OP_READ_CLIPBOARD` app-op must be allowed, and the device must
be unlocked (`isDeviceLocked` → `getPrimaryClip` returns `null` on the lock
screen). Crucially, that app-op check is only the first gate: an
`appops set <pkg> READ_CLIPBOARD allow` does not replace the later
`READ_CLIPBOARD_IN_BACKGROUND` / IME / focus test. Source:
[`ClipboardService.java`](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android10-release/services/core/java/com/android/server/clipboard/ClipboardService.java),
`clipboardAccessAllowed`, Android 10 (`android10-release`).

Three consequences that are widely got wrong and that decide this document:

- **An AccessibilityService is not on that list.** It never appears in
  `clipboardAccessAllowed`. An a11y service can read *text from view nodes* and
  can observe copy actions, which is where the folklore comes from, but it
  cannot call `getPrimaryClip()` from the background.
- **You cannot even learn that the clipboard changed.** `sendClipChangedBroadcast`
  runs the *same* `clipboardAccessAllowed` check per listener before dispatching,
  so `OnPrimaryClipChangedListener` is silent in the background. So is
  `getPrimaryClipDescription()`. There is no public background change signal.
- **`READ_CLIPBOARD_IN_BACKGROUND` is `signature`** — so `adb shell pm grant`
  cannot grant it to us. *But* `com.android.shell` declares it
  (`packages/Shell/AndroidManifest.xml`) and is platform-signed, so it holds it.
  **A binder call made as the shell UID with `callingPackage = "com.android.shell"`
  reads the clipboard in the background, with no focus and no overlay.** That is
  the hinge this whole document turns on.

### What changed after Android 10

- **Android 12 (API 31):** the first time an app calls `getPrimaryClip()` on
  another app's clip, the system toasts *"APP pasted from your clipboard."*
  `getPrimaryClipDescription()` does not toast.
  ([behaviour changes: all apps](https://developer.android.com/about/versions/12/behavior-changes-all))
  In source: `showAccessNotificationLocked`, suppressed for the default IME,
  ContentCapture, Autofill, and holders of `SUPPRESS_CLIPBOARD_ACCESS_NOTIFICATION`
  (`signature`; the shell package does **not** hold it), and shown at most once
  per (uid, clip).
- **Android 13 (API 33):** `LogcatManagerService`. An app with `READ_LOGS` that
  runs `logcat` now triggers a consent dialog; access is granted for a short
  window and **the dialog is only shown when the app is on top — background apps
  are denied automatically**.
  ([Android Help: manage your device logs](https://support.google.com/android/answer/12986432),
  [issuetracker 232206670](https://issuetracker.google.com/issues/232206670),
  [issuetracker 243904932](https://issuetracker.google.com/issues/243904932).
  *Marked: the 60-second window figure comes from those issue threads, not from
  official documentation.*) A clipboard monitor is in the background by
  definition, so unattended app-owned logcat access is not a supported platform
  guarantee.
- **Android 13 (API 33):** the clipboard auto-clears after a period.
- **Android 15 (API 35):** `SYSTEM_ALERT_WINDOW` no longer exempts a
  foreground-service start unless an overlay is actually visible. It **still**
  exempts background *activity* starts
  ([background starts](https://developer.android.com/guide/components/activities/background-starts)).
- **Android 17 (API 37):** the adb docs now describe "adb Wi-Fi 2.0", which
  reconnects automatically to trusted networks. *Marked as unverified* whether
  this removes the per-reboot restart for the on-device (localhost) case — see
  rung 2.

## 2. Current capture-state contract

The app reports exactly four states:

- `NOT_GRANTED`
- `DISABLED`
- `GRANTED_NOT_WORKING`
- `WORKING`

Permission presence alone never produces `WORKING`; only a successful read does
(`CopyPaste-qzhu`). Loss of the runtime reader clears that proof and surfaces the
fallback immediately. ClipCascade-named Kotlin is the current app-owned capture
path, not a compatibility layer.

## 3. The ladder

Ordered by what it costs the user. "Reboot" = does capture survive a restart
without the user doing anything.

| Rung | What the user does | Gets | Reboot | Our app update | Play | On grant loss |
|---|---|---|---|---|---|---|
| **0 — nothing** | nothing | copies made inside CopyPaste; anything sent via share sheet or the text-selection "Copy to CopyPaste" action (`ACTION_PROCESS_TEXT`); one tap on a Quick Settings tile captures whatever is on the clipboard right now (the tile gives our activity focus, so the read is legal); everything the Mac captured, over sync | ✅ | ✅ | ✅ | n/a — this is the floor |
| **1 — overlay** | one toggle: Settings → Display over other apps | a floating bubble the user taps after copying, without leaving the app they are in; also the background-activity-start exemption rung 2 does not need but rung 0's tile benefits from | ✅ | ✅ | ✅ (declare `specialUse` FGS) | `Settings.canDrawOverlays()` on every resume; app hibernation can revoke it |
| **2 — one-time setup** | use Shizuku on the phone or copy the provided adb commands on a computer | app-owned logcat and focused overlay capture; Shizuku can be removed after grants | new reader may need OS consent | existing live reader is reused | distribution policy must be reviewed separately | explicit recovery after reader loss |
| **3 — become the keyboard** | switch their keyboard to ours | the only *documented, supported, reboot-proof* background access | ✅ | ✅ | ✅ | user switches keyboard back |
| **Manual adb setup** | run the same permission commands from a computer | same runtime as rung 2 | same reader-consent constraint | same runtime | same policy | same recovery |

**Rejected outright.** *AccessibilityService*: not an exemption (§1), plus
Play's [AccessibilityService policy](https://support.google.com/googleplay/android-developer/answer/10964491)
requires either an `isAccessibilityTool` claim we are not entitled to or an
in-app prominent disclosure and affirmative consent — a large cost for a
mechanism that does not work. *NotificationListener*: sees notifications, never
the clipboard; not a route at all. *Root*: excluded by the brief.

**Rung 3 deserves one honest sentence.** Being the default IME is the only
mechanism Google actually intends for this, it survives reboots and updates, and
it suppresses the Android 12 toast. We are not shipping it because a clipboard
manager that requires you to change keyboards is a keyboard product, and a bad
keyboard loses the user more than background capture wins them. Worth
reconsidering only if rung 2 turns out to be unusable in practice.

## 4. Rung 2 in detail — app-owned reader

Shizuku is a setup helper. Both setup methods use the same native command list:

- `pm grant <pkg> android.permission.READ_LOGS`
- `cmd appops set <pkg> SYSTEM_ALERT_WINDOW allow`
- `cmd appops set <pkg> RUN_IN_BACKGROUND allow`
- `cmd appops set <pkg> RUN_ANY_IN_BACKGROUND allow`
- `am set-inactive <pkg> false`
- `am set-standby-bucket <pkg> active`

The computer instructions prefix each command with `adb shell`. Onboarding
saves its checkpoint before a grant can restart the process, then verifies
actual permissions rather than trusting a completion marker.

`CaptureService` owns a single app-UID `ClipboardService:E` logcat reader.
The reader matches only this application id and signals a focused overlay
read; it does not publish log contents. Reopening the UI reuses the live reader.
Shizuku is not needed afterward. Android log-access approval applies to the
open reader: process death, reboot or logd restart may require a new foreground
approval. See [Android 16 LogcatManagerService](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android16-release/services/core/java/com/android/server/logcat/LogcatManagerService.java).

Exact source-app attribution is unavailable without a privileged runtime.
Configured exclusions therefore fail closed before an implicit read; explicit
share, Process Text, tile and in-app capture remain available.

Physical validation must cover consent, reader reuse across UI closure,
reader loss, overlay focus and OEM battery policy.

## 5. What we tell the user when a grant disappears

This is the Android restatement of ADR-0001's lesson: a permission that silently
lapses turns a clipboard manager into a product that quietly saves nothing, and
the user finds out only when they go looking for something that is not there.
Silent failure is the worst outcome; **data the user believed was saved and was
not is worse than a visible refusal to capture.**

Binding rules for the Android UI:

1. **Capture state is visible wherever history is visible.** The history list
   carries a persistent, unmissable indication of which rung is live.
2. **"Working" means a read succeeded**, not that a permission is present.
   The four states above and `CopyPaste-qzhu` forbid optimistic success.
3. **Loss is pushed, not polled.** Register a binder death recipient on
   Shizuku; on death, post a notification — *"Background capture stopped.
   CopyPaste is only saving what you copy inside the app. Tap to restart."* —
   and flip the in-app state in the same instant. A reboot is the expected
   trigger, so this notification is a routine part of the product, not an error
   path.
4. **Re-arming is one tap from the notification**, landing on the rung-2 screen
   with the Start step pre-selected. This is the difference between "redo it
   every reboot" being an annoyance and being abandonment.
5. **Every item records how it arrived** — captured on this phone, captured in
   app, or synced from the Mac. Then a gap in the history is explainable rather
   than mysterious. Manifest 01's attribution requirement already covers the
   data model for this.
6. **Check every entry point, not just startup:** `Shizuku.pingBinder()`,
   `canDrawOverlays()`, and permission state re-evaluated on every `onResume`
   (`CopyPaste-qzhu` also forbids a cached result after returning from system
   Settings). Note also that
   [app hibernation](https://developer.android.com/topic/performance/app-hibernation)
   revokes permissions and force-stops apps unused for months — unlikely for
   this app, but the check is cheap.

## 6. Is sync-first an honest product?

**Yes — with one condition, and it is not a small one.**

Android as *consumer of the Mac's history plus a decent in-app capture surface*
is a real product. Cross-device clipboard is the actual reason someone installs
this: the value is "the thing I copied on my Mac is on my phone", and that
direction works perfectly with zero permissions. Rung 0 is not a degraded mode;
it is the majority of the value, delivered on install with nothing to configure.

The condition: **the app must never imply it is capturing when it is not.** The
broken promise is not "Android can't capture in the background" — users have
lived with that since 2019. The broken promise is a clipboard manager that looks
like it is running and is not. Concretely:

- Store listing and onboarding say what rung 0 does, in those words, before the
  user installs. Not "clipboard history for Android" full stop.
- Onboarding's last card offers rung 2 as *"Capture from other apps (advanced,
  needs a one-time setup and a re-tap after each restart)"* — visible, optional,
  and not the thing standing between the user and a working app.
- The Quick Settings tile is set up in onboarding, because it is the one action
  that makes rung 0 feel like a clipboard manager rather than a viewer.

The rung-2 setup screen, status surface, and loss notification ship with the
capture plumbing so the capability is usable from the product UI.

## Open questions

- Device spike for the Shizuku shell-UID clipboard path (§4). **Blocking** —
  the recommendation rests on it.
- Whether Android 17's "adb Wi-Fi 2.0" removes the per-reboot restart for the
  on-device localhost case.
- Whether Google Play has ever objected to a *clipboard* app integrating with
  Shizuku, as opposed to the debloaters and permission managers that do it today.
- OEM variance: whether Samsung/Xiaomi builds restrict the on-device wireless
  debugging pairing flow (Shizuku's own FAQ lists MIUI-specific failures).
