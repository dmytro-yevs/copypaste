# ADR-0005 — Android capture: decisions in Rust, facts from Kotlin

**Status:** accepted · 2026-07-30
**Scope:** how the four-rung ladder in
[`docs/rewrite/android-clipboard-access.md`](../rewrite/android-clipboard-access.md)
is built. That document is the specification and this one is the shape of the
implementation; where they disagree, it wins.
**Related:** [ADR-0002](0002-one-cross-platform-app.md) (one Tauri app),
[ADR-0003](0003-one-command-surface-two-backends.md).

## Decision

**Rust owns product policy; Kotlin enforces its pre-read projection.**

Rust remains the source of config, capture-origin semantics, state transitions,
user-facing text and consent. Kotlin reports platform facts and enforces one
synchronized projection: app exclusions before clipboard text is materialized.
Sending plaintext to Rust before that decision would itself violate manifest
I-7. The embedded backend repeats the source-aware gate at the write boundary,
so a stale or bypassed native bridge cannot persist unknown external capture.

This is ADR-0002's lesson applied to the one place the platform genuinely needs
native code. That ADR deleted ~2,500 lines of Kotlin because no machine in this
project could compile them. The line here is not "no Kotlin" — a Quick Settings
tile and a binder proxy cannot be written in Rust — it is **no decisions in the
part nothing can compile**. `capture::model` tests the state machine and the
wording. Kotlin's one contract test serialises its production DTOs into a
checked fixture that Rust consumes; it tests the bridge shape without moving
policy into Kotlin.

The same reasoning puts the loss notification's *wording* in Rust and its
*posting* in Kotlin: the text is passed down at arm time so the binder death
recipient can post it without Rust being scheduled, which matters because the
process may be going away.

## What is built

**Rung 0, complete on the Rust side and written on the Android side.** Three
doorways — the share sheet (`ACTION_SEND`), the text-selection action
(`ACTION_PROCESS_TEXT`) and a Quick Settings tile — all reach
`Backend::add` through `capture::intake`, which is the one ingest path
(`copypaste_core::ingest`). The tile's tap is what gives `IntakeActivity` focus,
and focus is the clipboard exemption we can reach with no permission at all.

**Rung 2 written, partially verified.** `ShizukuSettingsService` is only the
setup bridge: it applies the ClipCascade-style grants, standby relaxations and
the optional toast setting. The live reader is app-owned
(`ClipCascadeCapture` + `ClipboardFloatingActivity` + `CaptureService`).

**Rungs 1 and 3 are not built** and are not represented in the state model. An
overlay bubble and becoming the default IME are both in the specification's
ladder; neither is a state this code can be in, so neither has an enum variant
to mislead someone.

## Three decisions worth the words

**`Working` requires a read that happened without focus.** Any app may read the
clipboard while it is in front. So the read `arm` takes, and every read the tile
takes, prove that the clipboard is readable — not that it is readable in the
background, which is the only thing `Working` claims. Counting them would turn
the setup screen green at the exact moment it knows least. `CopyPaste-qzhu`
requires `record_read` to carry the `focused` fact.

**Kotlin owns the device-only runtime.** The app-owned reader (`ClipCascadeCapture`
plus `ClipboardFloatingActivity`) depends on logcat, overlay focus and Android
service rules that Rust cannot exercise on this host. Rust therefore receives
facts and clips, not callbacks into its own process.

**Kotlin queues and Rust drains once a second.** This is not clipboard polling:
the clipboard signal is produced on the Kotlin side, and the drain only moves
already captured text between two halves of one process. The alternative was a
JNI callback, which needs `unsafe` in a crate that forbids it. The cost is up
to a second of latency to storage and a loss window if the process dies with
clips queued — `Buffer` counts what it loses and the count is surfaced, because
a copy that was not saved is precisely what the user must not have to discover
for themselves.

**App exclusions run before the clipboard read.** Android exposes the writer
package only through hidden `getPrimaryClipSource` from API 31, guarded by the
signature-level `SET_CLIP_SOURCE` permission. The existing maintained Shizuku
client supplies the shell identity; Kotlin resolves the package and cached
label before asking for `primaryClip`. With exclusions configured, an excluded
or unavailable source skips only implicit background capture. Share, Process
Text, the tile and the in-app action remain explicit user-directed intake.

No maintained package exposes this hidden clipboard-service method or its
versioned signature. This is dependency rule exemption 1: the bridge keeps the
AOSP API 31–33 and API 34+ signatures at the platform boundary, while Shizuku
continues to own binder identity and transport.

The bridge targets the clipboard that owns the reading context, not user 0 or
the default device. The numeric user ID mirrors AOSP
[`UserHandle.getUserId(Process.myUid())`](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android16-release/core/java/android/os/UserHandle.java),
whose numeric accessor is hidden from the public SDK. On API 34+ the bridge
also passes public [`Context.getDeviceId()`](https://developer.android.com/reference/android/content/Context#getDeviceId()),
so work-profile and virtual-device clipboards cannot silently resolve source
metadata from a different clipboard silo.

## What this creates for other people

* **Attribution is device-level, not surface-level.** `Item` now carries
  `origin_device_id`, so "from the Mac" versus "from this phone" is renderable.
  Which *doorway* an item came through — share sheet, tile, background — travels
  only on the `copypaste://captured` event and is not persisted. Persisting it
  needs a column and an `Item` field, and is worth deciding rather than
  assuming.
* **Settings persist locally on Android.** The embedded backend atomically
  writes `settings-v2.json`; an unreadable record starts with private mode,
  sync and LAN visibility disabled instead of silently restoring defaults.
* **The React side owns the surfaces.** Commands and events exist
  (`capture_state`, `capture_arm`, `capture_now`,
  `capture_set_toast_suppressed`, `copypaste://capture-state`,
  `copypaste://captured`); the rung 2 setup screen, the status indicator beside
  history and the toast-consent dialog are not written. AGENTS.md rule 6 is
  therefore **not** satisfied yet, and this is the gap to close first.

## Unverified

The bridge DTOs compile in Android debug unit-test variants, and their fixture
guard is attached to debug APK assembly. Rung 2 still needs pairing and capture
evidence on a real phone; `docs/rewrite/android-spike.md` remains its checklist.
