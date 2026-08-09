# ADR-0005 — Android capture: decisions in Rust, facts from Kotlin

**Status:** accepted · 2026-07-30
**Scope:** how the four-rung ladder in
[`docs/rewrite/android-clipboard-access.md`](../rewrite/android-clipboard-access.md)
is built. That document is the specification and this one is the shape of the
implementation; where they disagree, it wins.
**Related:** [ADR-0002](0002-one-cross-platform-app.md) (one Tauri app),
[ADR-0003](0003-one-command-surface-two-backends.md).

## Decision

**Kotlin reports facts. Rust decides what they mean.**

`crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/` contains no
policy: is Shizuku running, did a read return, here is the text. Every state
transition, every sentence the user reads, and the consent gate on the Android
12+ notice live in `src-tauri/src/capture/model.rs`, which compiles and is
tested on a Linux host with no Android SDK.

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

**Rung 2 written, unverified.** `ShizukuClipboard` obtains the clipboard binder
through `ShizukuBinderWrapper` and calls it as `com.android.shell`.

**Rungs 1 and 3 are not built** and are not represented in the state model. An
overlay bubble and becoming the default IME are both in the specification's
ladder; neither is a state this code can be in, so neither has an enum variant
to mislead someone.

## Three decisions worth the words

**`Working` requires a read that happened without focus.** Any app may read the
clipboard while it is in front. So the read `arm` takes, and every read the tile
takes, prove that the clipboard is readable — not that it is readable in the
background, which is the only thing `Working` claims. Counting them would turn
the setup screen green at the exact moment it knows least. This is
`CopyPaste-qzhu` in a subtler costume than the one v1 shipped, and
`record_read` takes a `focused` flag because of it.

**`IClipboard` is called reflectively, and only its callback is AIDL.** Its
signatures have gained parameters repeatedly — `attributionTag` in API 30,
`deviceId` in 34 — so a compiled-in AIDL is a guess that breaks on the next
release. `ShizukuClipboard.invoke` fills the argument vector from the method's
own parameter types. The single exception is
`IOnPrimaryClipChangedListener`, declared as AIDL because a Binder stub cannot
be produced by reflection; it has been one no-argument callback since it was
introduced.

**Kotlin queues and Rust drains once a second.** This is not clipboard polling:
the clipboard signal is the listener's push, and the drain only moves already
captured text between two halves of one process. The alternative was a JNI
callback, which needs `unsafe` in a crate that forbids it. The cost is up to a
second of latency to storage and a loss window if the process dies with clips
queued — `Buffer` counts what it loses and the count is surfaced, because a copy
that was not saved is precisely what the user must not have to discover for
themselves.

## What this creates for other people

* **Attribution is device-level, not surface-level.** `Item` now carries
  `origin_device_id`, so "from the Mac" versus "from this phone" is renderable.
  Which *doorway* an item came through — share sheet, tile, background — travels
  only on the `copypaste://captured` event and is not persisted. Persisting it
  needs a column and an `Item` field, and is worth deciding rather than
  assuming.
* **Settings do not persist on Android.** The embedded backend uses
  `ConfigData::default()`; there is no config file and no daemon to hold one, so
  a settings screen there will not stick.
* **The React side owns the surfaces.** Commands and events exist
  (`capture_state`, `capture_arm`, `capture_now`,
  `capture_set_toast_suppressed`, `copypaste://capture-state`,
  `copypaste://captured`); the rung 2 setup screen, the status indicator beside
  history and the toast-consent dialog are not written. CLAUDE.md rule 6 is
  therefore **not** satisfied yet, and this is the gap to close first.

## Unverified

The bridge DTOs compile in Android debug unit-test variants, and their fixture
guard is attached to debug APK assembly. Rung 2 still needs pairing and capture
evidence on a real phone; `docs/rewrite/android-spike.md` remains its checklist.
