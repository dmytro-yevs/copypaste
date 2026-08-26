# ADR-0002 — One cross-platform app: Tauri v2 + React

**Status:** accepted · 2026-07-30 · **reverses the native-app decision**
**Scope:** what the product surface is on macOS and Android.

## Decision

There is **one app**, built with **Tauri v2 and React**, shipping to both
macOS and Android. `crates/copypaste-ui` is that app.

This reverses a decision taken earlier the same day to write two native apps —
SwiftUI on macOS, Jetpack Compose on Android — with a UniFFI crate bridging
Rust to Kotlin and Swift. That work is deleted, not archived in place:
`apps/macos/` (34 Swift files, ~4,500 lines), `apps/android/` (~2,500 lines of
Kotlin) and `crates/copypaste-ffi/` (~1,900 lines) are gone. They remain in
git history at `2cbeef3b` if anything needs recovering.

## Why the reversal

The native decision optimised for the wrong thing. It bought platform fidelity
— SwiftUI materials, Compose Material 3 — at the price of writing the history
list, the search field, the pairing flow, the settings screen and the error
states **three times**: once in Swift, once in Kotlin, and once already in
React. Two of those three were never compiled, because this host has neither
Xcode nor the Android SDK, so the cost was being paid in code that nobody could
run.

That is the failure this rewrite exists to end, arriving in a new costume.
AGENTS.md rule 1 is about not writing what a library already provides; the same
logic applies to writing the same screen twice because two platforms are in
scope. Three implementations of one history list is the UI-layer version of six
retry implementations.

The honest accounting of what was lost by reversing: native scroll physics,
platform-native accessibility for free rather than by construction, and a
smaller binary. The honest accounting of what was gained: one implementation of
every screen, and a UI that can actually be built and tested on the machine the
work happens on.

## What carries over unchanged

**ADR-0001 still holds in full.** A Tauri app on macOS is still a `.app`
bundle, still ad-hoc signed, still distributed through our own Homebrew tap
with a `postflight` that strips the quarantine attribute. Nothing about the
signing analysis depended on the UI toolkit.

**The zero-TCC-permission constraint still holds, and still binds.** Reading
`NSPasteboard` is the daemon's job and needs no permission; selecting an item
must put it on the clipboard rather than synthesise Cmd+V.

### The global-hotkey permission question — settled

This was recorded here as an open question. The crate has now been read and the
answer is **conditional, not yes or no**: the permission cost is not a property
of the plugin, it is a property of **which key is registered**.

`tauri-plugin-global-shortcut` 2.3.2 depends on `global-hotkey` 0.8.0. In
`global-hotkey-0.8.0/src/platform_impl/macos/mod.rs`:

| line | what it does | permission |
|---|---|---|
| 44 `GlobalHotKeyManager::new` | Carbon `InstallEventHandler` only. No tap is created at startup. | none |
| 116 `register` | **Carbon `RegisterEventHotKey`**, for every key with an entry in `key_to_scancode` (line 411) — all letters, digits, function keys, arrows, Space, Escape, and even the volume keys at 0x48–0x4a | **none** |
| 140 → 206 `start_watching_media_keys` | **`CGEventTapCreate`** at `CGEventTapLocation::Session` with `CGEventTapOptions::Default` — an *active* tap, not a listener | **Accessibility** |

The tap branch is reached only when `key_to_scancode` returns `None` *and*
`is_media_key` (line 522) is true. That is exactly five keys:
`MediaPlayPause`, `MediaTrackNext`, `MediaTrackPrevious`, `MediaFastForward`,
`MediaRewind`.

**So ADR-0001's property holds for every shortcut we would plausibly ship, and
is forfeited by five keys.** The FFI declarations back this up:
`platform_impl/macos/ffi.rs` links `Carbon` (line 90) for
`RegisterEventHotKey` and `CoreGraphics` (line 216) for `CGEventTapCreate`.
`NSEvent.addGlobalMonitorForEvents` is not used anywhere in the crate — that
half of the conflicting reports was simply wrong.

**Why this needed a guard rather than a note.** The plugin hands shortcut
strings straight to `Shortcut::from_str`, and `global-hotkey`'s parser accepts
`MEDIAPLAYPAUSE` and its siblings (`hotkey.rs` lines 335–340). The moment the
app grows a user-configurable shortcut recorder, a user picking a media key
would create a `CGEventTap`, raise a TCC prompt, and then lose the grant on the
next `brew upgrade` — the hotkey silently dying after an update, which is the
precise outcome this ADR said not to ship.

`crates/copypaste-ui/src-tauri/src/shell/hotkey.rs` therefore refuses those five
keys at registration, keyed on `Code` rather than on the string spelling, with
the citation above in its module docs and a test per key. The default shortcut
is Cmd+Shift+V, which takes the Carbon path.

**What is still unverified.** The source reading is exact; the *behaviour* is
inferred from the API used and has never been observed. No build on this host
has called `RegisterEventHotKey` — there is no macOS SDK here. Confirming that
no TCC prompt appears, and that the hotkey survives a `brew upgrade`, needs a
Mac and is the first thing to check on one.

**If upstream changes.** The guard is pinned to a crate version. If
`global-hotkey` ever moves the ordinary-key path onto a tap, the guard will not
notice — it only knows about the five keys. Re-read
`platform_impl/macos/mod.rs` when that dependency is bumped.

**Manifest 06's behaviour half remains binding**, exactly as
`docs/rewrite/port-manifest/README.md` says: scroll anchoring, the row-height
over-reservation rule, the 15 accessibility requirements, sensitive content
absent from the view rather than obscured, no filesystem path in any
user-facing error. A webview does not get to drop these any more than a native
app did — it just has to satisfy them with DOM and ARIA instead of SwiftUI and
TalkBack.

`design/dist/` is generated from the current DTCG tokens and is not an authoring
source. Visual changes begin in `design/tokens/` and pass the contrast and usage
gates before generated CSS changes.

## Android has no daemon, and that does not change

The reason `copypaste-ffi` existed is still true: Android will not host a
long-lived background daemon, so the Android build cannot talk to one over a
Unix socket the way the desktop build does. What changes is the binding layer,
not the architecture — the Rust side of the Tauri app embeds `copypaste-core`
and `copypaste-p2p` directly and exposes them as Tauri commands, instead of
UniFFI generating Kotlin for a Compose app to call.

So the Tauri bridge needs two backends behind one command surface:

| target | backend |
|---|---|
| macOS | IPC to the running daemon over the `0600` Unix socket |
| Android | the core linked into the app process |

That seam is the main piece of real work this decision creates, and it is
where the next agent should start. Keeping the command surface identical
across both is what stops the React side from growing platform branches.

**The seam now exists** — `crates/copypaste-ui/src-tauri/src/backend/`, one
`Backend` trait with two implementations, selected by a compile-time type
alias. What it uncovered is that the Android half cannot be finished from
inside `copypaste-ui`: the ingest pipeline and the p2p node both live in
`copypaste-daemon`, which is a binary with no `[lib]` target. See ADR-0003.

## What would change this

A platform capability that a webview genuinely cannot reach. The macOS menu-bar
item and the global hotkey were the obvious candidates; both have Tauri plugins
and both are now wired, and the hotkey's permission question is settled above.
If a future feature needs something no plugin provides, the answer is a small
native plugin behind a Tauri command — not a second full app.
