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
CLAUDE.md rule 1 is about not writing what a library already provides; the same
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
must put it on the clipboard rather than synthesise Cmd+V. One item needs
verification that the native decision had settled and this one reopens:

> **Open question.** `tauri-plugin-global-shortcut` delegates to the
> `global-hotkey` crate. If that uses Carbon `RegisterEventHotKey` on macOS,
> the hotkey needs no Accessibility permission and ADR-0001's property is
> intact. If it uses `NSEvent.addGlobalMonitorForEvents` or a `CGEventTap`,
> it requires Accessibility — and per ADR-0001 an ad-hoc-signed app loses that
> grant on every update. Sources conflict. **Read the crate before shipping a
> hotkey**, and if it takes the monitor path, either contribute the Carbon path
> upstream or bind the hotkey through a small plugin of our own. Do not ship a
> hotkey that silently stops working after an upgrade.

**Manifest 06's behaviour half remains binding**, exactly as
`docs/rewrite/port-manifest/README.md` says: scroll anchoring, the row-height
over-reservation rule, the 15 accessibility requirements, sensitive content
absent from the view rather than obscured, no filesystem path in any
user-facing error. A webview does not get to drop these any more than a native
app did — it just has to satisfy them with DOM and ARIA instead of SwiftUI and
TalkBack.

**v1's visual design is still rejected.** `design/dist/` holds v1's palette
value-for-value; it is not a fallback. The new look is still an undecided
decision, and re-deriving v1's tokens by importing that directory is the
outcome the manifest README warns against.

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

## What would change this

A platform capability that a webview genuinely cannot reach. The macOS menu-bar
item and the global hotkey are the obvious candidates; both have Tauri plugins,
and the hotkey's permission question above is the one to settle first. If a
future feature needs something no plugin provides, the answer is a small native
plugin behind a Tauri command — not a second full app.
