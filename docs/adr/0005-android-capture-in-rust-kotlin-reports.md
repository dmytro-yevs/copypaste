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

**Rung 2 uses one-time setup grants.** Shizuku applies the same fixed permission
commands shown by the manual adb setup. It is not a runtime dependency.
`CaptureService` owns one app-UID logcat reader and its focused overlay hand-off.
Only occurrence signals leave the reader; raw logs never enter history or IPC.
Android may ask for log-access consent when a new reader starts, but reopening
the activity reuses an existing reader.

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

**Kotlin owns the device-only runtime.** `ClipCascadeCapture` owns the process,
reader and generation fence. Stop, replacement and queued callbacks are scoped
to their run so old work cannot restart capture or announce a false loss.

**Kotlin queues; Rust drains on notification.** The maintained Tauri channel
wakes the Rust intake worker. Startup replay and a bounded fallback recover
missed notifications; an empty idle queue does not require a polling loop.

**App exclusions run before the clipboard read.** Exact source attribution
requires Android's signature-level `SET_CLIP_SOURCE` permission. A one-time
adb grant cannot provide it. Without a source, configured exclusions skip
implicit background reads. Explicit share, Process Text, tile and in-app
capture remain available. The app never guesses the source or silently drops
an existing exclusion rule.

No maintained package exposes a product-specific, content-free interpretation
of ClipboardService log events. This is dependency-rule exemption 1 for the
small app-owned reader: it accepts only the fixed ClipboardService filter and
matches this application id. It does not expose arbitrary shell execution.
