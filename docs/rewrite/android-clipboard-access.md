# Android clipboard access — what the platform allows, and which rung we ship first

**Status:** proposed · 2026-07-30
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
2. **Rung 2 (Shizuku) is the one upgrade we build.** It is the only path that
   gives real background capture on an unrooted phone with no computer, and it
   is a materially better mechanism than v1's — it reads the clipboard
   *directly* as the shell UID, with a real change listener, instead of tailing
   logcat for a denial message.
3. **We do not port v1's `READ_LOGS`/logcat approach.** It is structurally dead
   on Android 13+ (see below), and it was already reporting itself as broken in
   v1.
4. **We do not build an AccessibilityService.** Contrary to a decade of folklore
   — including v1's own UI strings — it is *not* a clipboard exemption in AOSP.
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
  cannot call `getPrimaryClip()` from the background. v1's settings screen told
  users "Enable AccessibilityService instead" — that advice pointed at a service
  v1 never even implemented, and it would not have worked.
- **You cannot even learn that the clipboard changed.** `sendClipChangedBroadcast`
  runs the *same* `clipboardAccessAllowed` check per listener before dispatching,
  so `OnPrimaryClipChangedListener` is silent in the background. So is
  `getPrimaryClipDescription()`. This is the whole reason v1 was reduced to
  reading logcat: there is no legitimate change signal at all.
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
  definition, so this closes v1's route completely.
- **Android 13 (API 33):** the clipboard auto-clears after a period.
- **Android 15 (API 35):** `SYSTEM_ALERT_WINDOW` no longer exempts a
  foreground-service start unless an overlay is actually visible. It **still**
  exempts background *activity* starts
  ([background starts](https://developer.android.com/guide/components/activities/background-starts)).
- **Android 17 (API 37):** the adb docs now describe "adb Wi-Fi 2.0", which
  reconnects automatically to trusted networks. *Marked as unverified* whether
  this removes the per-reboot restart for the on-device (localhost) case — see
  rung 2.

## 2. What v1 did

`archive/v0.4.1-pre-rewrite`, `android/app/src/main/java/com/copypaste/android/`:

- **Permission:** `android.permission.READ_LOGS`
  (`signature|privileged|development` — the `development` flag is why shell can
  grant it), declared in the manifest so the grant is accepted.
- **Command:** `adb shell pm grant com.copypaste.android android.permission.READ_LOGS`,
  plus `adb shell appops set … SYSTEM_ALERT_WINDOW allow` and
  `adb shell am force-stop …`, shown as three tap-to-copy rows in Settings.
- **Mechanism** (`LogcatCaptureService.kt`, ~500 lines): tail
  `logcat ClipboardService:E *:S`, match the *denial* line naming our own
  package, debounce 1000 ms, then launch `ClipboardFloatingActivity` — a
  transparent overlay activity that clears `FLAG_NOT_FOCUSABLE`, waits for the
  layout pass, and reads the clip in the focus window. Copied from ClipCascade.
- **Detection:** `checkSelfPermission(READ_LOGS)`, plus a `logcatCaptureWorking`
  flag set only when a read actually returned a clip, yielding four states:
  `NOT_GRANTED / DISABLED / GRANTED_NOT_WORKING / WORKING`.
- **On loss:** the logcat stream ending set `logcatCaptureWorking = false` and
  stopped the service. The user saw *"Status: granted but not working — Android
  11+ may scope system logs. Enable AccessibilityService instead."* — buried in
  Settings › Diagnostics, with no notification, pointing at a feature that did
  not exist.

Two things to carry forward and one to bury. Carry: the four-state status model,
and the discipline of only reporting "working" after a read actually succeeded
(v1 fixed a real bug, `CopyPaste-qzhu`, by removing an optimistic
`working = true`). Bury: everything else.

## 3. The ladder

Ordered by what it costs the user. "Reboot" = does capture survive a restart
without the user doing anything.

| Rung | What the user does | Gets | Reboot | Our app update | Play | On grant loss |
|---|---|---|---|---|---|---|
| **0 — nothing** | nothing | copies made inside CopyPaste; anything sent via share sheet or the text-selection "Copy to CopyPaste" action (`ACTION_PROCESS_TEXT`); one tap on a Quick Settings tile captures whatever is on the clipboard right now (the tile gives our activity focus, so the read is legal); everything the Mac captured, over sync | ✅ | ✅ | ✅ | n/a — this is the floor |
| **1 — overlay** | one toggle: Settings → Display over other apps | a floating bubble the user taps after copying, without leaving the app they are in; also the background-activity-start exemption rung 2 does not need but rung 0's tile benefits from | ✅ | ✅ | ✅ (declare `specialUse` FGS) | `Settings.canDrawOverlays()` on every resume; app hibernation can revoke it |
| **2 — Shizuku + ClipCascade grants** ⭐ | install Shizuku (Play); Developer options → Wireless debugging; pair once with a code; tap Start; grant CopyPaste's Shizuku permission once | **full background capture from every app** through CopyPaste's own logcat + overlay path after one-shot grants | ✅ after setup | ✅ | Shizuku is on Play; nothing in policy prohibits using it as a setup bridge | `READ_LOGS`, overlay, battery policy and OEM logcat behaviour still need device evidence |
| **3 — become the keyboard** | switch their keyboard to ours | the only *documented, supported, reboot-proof* background access | ✅ | ✅ | ✅ | user switches keyboard back |
| ~~4 — adb from a computer~~ | plug into a Mac, paste `pm grant … READ_LOGS` | **nothing, on Android 13+** | — | — | — | — |

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

## 4. Rung 2 in detail — Shizuku as the setup bridge

**How it works.** Shizuku is not the live clipboard transport. It is the
one-shot setup bridge that applies the grants CopyPaste's own ClipCascade path
needs:

- `pm grant <pkg> android.permission.READ_LOGS`
- `cmd appops set <pkg> SYSTEM_ALERT_WINDOW allow`
- `cmd appops set <pkg> RUN_IN_BACKGROUND allow`
- `cmd appops set <pkg> RUN_ANY_IN_BACKGROUND allow`
- `am set-inactive <pkg> false`
- `am set-standby-bucket <pkg> active`
- `am force-stop <pkg>` so the new state takes effect cleanly

After that, CopyPaste runs the runtime path as itself: `ClipCascadeCapture`
tails logcat for the clipboard-denial line naming our package, launches
`ClipboardFloatingActivity`, and reads the clipboard only after the overlay
window has focus.

What Shizuku can persist for us is narrower than "background clipboard access".
Its user-service may set app-ops and standby state:

- `cmd appops set <pkg> RUN_IN_BACKGROUND allow`
- `cmd appops set <pkg> RUN_ANY_IN_BACKGROUND allow`
- `am set-inactive <pkg> false`
- `am set-standby-bucket <pkg> active`

It may also write the clipboard-toast setting. Shizuku may quit afterwards; the
runtime reader is CopyPaste's own process. One privacy feature is intentionally
stricter: while app exclusions are non-empty, CopyPaste needs Shizuku running
to ask the API 31+ clipboard service which package wrote the clip. If that
source cannot be resolved, implicit background capture skips the event before
reading its content. Explicit share, Process Text, tile and in-app actions do
not depend on attribution.

> **Partially verified.** The API 36 emulator leg proves the app-owned tile
> capture, the fail-closed service state, and the static grant path. The
> remaining unknowns are the device-only ones: `READ_LOGS`, overlay focus, OEM
> logcat behaviour, and battery managers.

**What the user installs.** [Shizuku](https://github.com/RikkaApps/Shizuku),
Apache-2.0, ~28k stars, on Google Play as `moe.shizuku.privileged.api`; the
[Shizuku-API](https://github.com/RikkaApps/Shizuku-API) client library is MIT.
Maintenance: not archived, but the last commit to either repo is mid-2025 —
**over a year quiet as of today**. That is a real risk to state: our best
background path depends on a third-party app with one maintainer.

**Does it need a PC?** No, on Android 11+.
[Official setup](https://github.com/RikkaApps/websites/blob/master/shizuku/guide/setup.md):
*"Starting with wireless debugging works on Android 11 or above. This startup
method does not require a connection to a computer."* The phone enables
Wireless debugging in Developer options and Shizuku pairs against the device's
own adb daemon over localhost. Android 10 and below still need a computer.

**Does it survive a reboot?** **No** — the same page: *"Due to system
limitations, the startup steps need to be performed again after each reboot."*
Pairing is once; **starting is every boot**. Reported to break on Wi-Fi network
changes too ([Shizuku #864](https://github.com/RikkaApps/Shizuku/issues/864) —
*marked, community report, not verified*). This is the single biggest cost of
rung 2 and the UI must be built around it, not apologise for it afterwards.

**The toast.** Every new clip we read as shell produces *"Shell pasted from your
clipboard"* once. It can be turned off system-wide
(`Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS = 0`), which shell can do —
**offer it as an explicit, explained opt-in and never do it silently.** Turning
off one of the OS's privacy indicators on the user's behalf is precisely the
move a clipboard manager must not make.

## 5. What we tell the user when a grant disappears

This is the Android restatement of ADR-0001's lesson: a permission that silently
lapses turns a clipboard manager into a product that quietly saves nothing, and
the user finds out only when they go looking for something that is not there.
Silent failure is the worst outcome; **data the user believed was saved and was
not is worse than a visible refusal to capture.**

Binding rules for the Android UI:

1. **Capture state is visible wherever history is visible.** Not buried in a
   diagnostics screen the way v1 buried it. The history list carries a
   persistent, unmissable indication of which rung is live.
2. **"Working" means a read succeeded**, not that a permission is present.
   Carry v1's four states (`NOT_GRANTED / DISABLED / GRANTED_NOT_WORKING /
   WORKING`) and its `CopyPaste-qzhu` fix: never optimistically report working.
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
   (v1's audit already caught a `remember(ctx)` that never refreshed after a
   trip to system Settings). Note also that
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

Per AGENTS.md rule 6, none of that is a later phase: the rung-2 setup screen,
the status surface, and the loss notification ship in the same stretch of work
as the capture plumbing. Pairing is the example that rule was written from, and
this is the same shape of feature.

## Open questions

- Device spike for the Shizuku shell-UID clipboard path (§4). **Blocking** —
  the recommendation rests on it.
- Whether Android 17's "adb Wi-Fi 2.0" removes the per-reboot restart for the
  on-device localhost case.
- Whether Google Play has ever objected to a *clipboard* app integrating with
  Shizuku, as opposed to the debloaters and permission managers that do it today.
- OEM variance: whether Samsung/Xiaomi builds restrict the on-device wireless
  debugging pairing flow (Shizuku's own FAQ lists MIUI-specific failures).
