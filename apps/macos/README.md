# CopyPaste for macOS

A menu-bar clipboard manager: an `NSStatusItem` with a popover, a Devices
window, and a Settings window. Swift 6, SwiftUI, AppKit where SwiftUI cannot
reach.

---

## ⚠️ None of this has ever been compiled or run

**Read this section before anything else.** This app was written on a Linux host
with no Swift toolchain, no macOS SDK and no Mac. Nothing here has been built,
launched, screenshotted, or tested against a running service.

What that means concretely:

- **Zero lines have been executed.** Not the app, not the unit tests, not the
  `Makefile`. `swift build` has never run.
- The **`Makefile` is a proposal**, not a tested build script. It is written to
  be readable and checkable, not to be trusted.
- Every claim below about *behaviour* describes what the code is written to do,
  not what has been observed.

The only check that was run is a **delimiter-balance scan** over the Swift
sources (matching braces, brackets, parens and quotes per file). It catches a
truncated or mis-pasted file. It does **not** prove the code parses, type-checks,
compiles, links, or works. Do not report it as a build.

### What a reviewer on a Mac should check first

In this order, because each one gates the next:

1. **`swift build`.** The likeliest failures are API drift in things this code
   cannot verify: `.scrollPosition(id:anchor:)`, `AccessibilityNotification.Announcement`,
   `@Bindable` on a passed-in `@Observable`, and the `KeyboardShortcuts` package
   API (see *Dependencies*).
2. **`swift test`.** `DaemonClientTests` runs a real `AF_UNIX` stub listener. If
   a test **hangs** rather than fails, look at `UnixSocketChannel` first — the
   `SO_RCVTIMEO` timeout is what bounds every read, and a hang means it is not
   taking effect.
3. **One live round trip per method.** The single highest-risk assumption in the
   whole app is the request envelope; see *The wire contract* below. Start the
   real service and exercise Status, List, Search, Copy, Add, Delete, DeleteAll,
   Pin, PairCreate, PairAccept, Unpair, Peers, SyncNow. A method the service
   rejects will say so in `error_code`, so this is quick.
4. **Scroll anchoring** — copy something while the popover is open and scrolled
   down. The row under the cursor must not move (manifest 06 INV-1).
5. **A sensitive item.** Copy something the detector flags (an API key), then
   open the popover. The row must show "Sensitive item — hidden", and Xcode's
   Accessibility Inspector must show no plaintext anywhere in the row.

### Assumptions I could not verify, ranked by what they would cost

| # | Assumption | If it is wrong |
|---|---|---|
| 1 | Serde's adjacently-tagged enum, flattened, emits **no `params` key** for a unit variant, and accepts the same on the way in | `status`, `delete_all` and `peers` fail with `invalid_request`. Fix is one line in `Method.encodeParams`: send `"params": null` instead |
| 2 | `ScrollView` + `.scrollPosition(id:)` keeps the bound row in place across a prepend | The scroll-anchoring invariant is not met and needs a different mechanism (`List` + explicit offset restoration) |
| 3 | The `KeyboardShortcuts` API is `Name(_:default:)` / `onKeyUp(for:)` / `Recorder(_:name:)` at the version SwiftPM resolves | `GlobalHotkey.swift` fails to compile. It is the only file that imports the package |
| 4 | `NSPopover` from an accessory app takes key focus after `NSApp.activate` | The search field cannot be typed into until clicked |
| 5 | A `@Sendable @MainActor` closure may capture a main-actor-isolated class (SE-0434) | `GlobalHotkey.onToggle`'s signature needs adjusting |
| 6 | `directories` (Rust) resolves `data_dir` to `~/Library/Application Support/com.copypaste.CopyPaste` | The socket is never found. `SocketPathTests` asserts our half; the other half is `copypaste_ipc::socket_path()` |

---

## Architecture

The app **embeds none of the Rust core**. No crypto, no storage, no detection,
no clipboard access. It is an IPC client, exactly like `copypaste-cli`:

```
CopyPaste.app  ──newline-delimited JSON──▶  ~/Library/Application Support/
 (SwiftUI/AppKit)      over AF_UNIX          com.copypaste.CopyPaste/daemon.sock
                                                      │
                                             copypaste-daemon (Rust)
```

The contract is `crates/copypaste-ipc/src/lib.rs`, and `crates/copypaste-cli/src/client.rs`
is the working client this one mirrors — including its `not_ready` retry
(3 attempts, 500 ms, reconnecting each time) and its rule that a reply whose
`id` does not match is a failure, never an answer.

```
Sources/
  CopyPasteKit/          no AppKit, no views — testable without a screen
    IPC/                 wire types, socket, client, error mapping, redaction
    Model/               ClipItem, PeerSummary, PairingSecret
    Services/            preferences, starting the service
    Stores/              observable state for history and devices
  CopyPasteApp/          AppKit shell + SwiftUI views
```

### The wire contract

`Method` mirrors the Rust enum's serde attributes rather than guessing:
`#[serde(tag = "method", content = "params", rename_all = "snake_case")]`,
flattened into `Request`. So:

```json
{"id":7,"protocol_version":1,"method":"list","params":{"limit":200,"offset":0}}
{"id":8,"protocol_version":1,"method":"status"}
```

Unit variants carry **no `params` key**, because that is what serde emits and
the service round-trips its own output. That is assumption #1 above.

`ResponseData` is `#[serde(untagged)]` on the Rust side, so decoding tries the
variants in declaration order. One consequence is inherited verbatim from the
CLI, comment and all: an **empty JSON array** matches `Items` before it can
reach `Peers`, so "no peers" and "no sync results" are accepted in both
spellings. `WireProtocolTests` pins this.

---

## The app requires no system permissions

**This is a requirement, not a happy accident. Do not regress it.**

CopyPaste ships **ad-hoc signed** — there is no paid Apple Developer ID. An
ad-hoc signature has no Team ID, so TCC keys a permission grant to the binary's
*cdhash*, which changes on every single build. Any permission the user granted
would therefore be revoked by every update and have to be granted again by hand.
For a clipboard manager that is a broken product. The only viable answer is to
need nothing.

Three constraints keep it that way:

1. **Clipboard reading** is `NSPasteboard` polling in the Rust service, which
   needs no TCC grant. Never reach for an event tap to detect ⌘C.
2. **The global hotkey** goes through Carbon `RegisterEventHotKey` (via
   `KeyboardShortcuts`), the one public global-hotkey API that does *not*
   require Accessibility: the app learns about the single combination it
   registered and never sees other input. Never switch to
   `NSEvent.addGlobalMonitorForEvents` or `CGEvent.tapCreate` — both require
   Accessibility. One consequence to accept rather than work around:
   `RegisterEventHotKey` cannot bind modifier-only shortcuts (double-tap
   Control), so the recorder does not offer them.
3. **Paste-back does not exist.** Writing to the pasteboard is free;
   synthesising ⌘V with `CGEventPost` is not — it requires Accessibility.
   Selecting an item puts it on the pasteboard and stops there, and the UI says
   so ("↩ copy · then ⌘V to paste"). **No text in this app may imply
   auto-paste.**

There is no `AXIsProcessTrusted` call, no permission prompt and no synthetic
event anywhere in this target. `Info.plist` carries no usage-description keys,
and the reason is written into the file so nobody adds one casually.

**On the horizon, not built for:** macOS 16 adds a user-facing alert when an app
reads the general pasteboard programmatically, plus an
`NSPasteboard.accessBehavior` property to request always-allow. That is a new
grant arriving for every clipboard manager regardless of signing, and it is
*not* the TCC problem above. It belongs next to the polling loop, which lives in
`crates/copypaste-daemon/src/capture.rs` — outside this directory, so the note
is here instead.

**The App Sandbox is off, and must stay off.** The socket lives in the real
`~/Library/Application Support`; a sandboxed process gets a redirected container
and there is no exception that grants a Unix domain socket outside it. See
`App/CopyPaste.entitlements`, which says so at the point someone would change
it.

---

## Design

The v1 look is rejected, so this app does not carry a palette over — and it does
not invent one either. `docs/rewrite/port-manifest/README.md` is explicit that
"visual is reference" is not "visual is undecided by default", and that an
implementation quietly re-deriving v1's palette is the outcome the decision
exists to prevent.

So this app uses **system semantics only**: `.primary` / `.secondary` /
`.tertiary` / `.quaternary`, `.selection`, `.tint`, materials, SF Symbols, and
`Form`/`GroupBox` where AppKit already has an opinion. It picks no hues and
hard-codes no hex values. It therefore inherits light mode, dark mode, increased
contrast, reduced transparency and the user's accent colour for free, and when
the new design lands there is nothing to un-pick.

**`design/dist/` is deliberately not consumed here.** Those tokens are
value-for-value v1's palette (its own README says the generated CSS was verified
declaration-by-declaration against `design-reference.html`). Importing them
would be exactly the re-derivation the manifest warns about. If a v2 palette is
decided later, this is the seam where it goes in.

---

## What the manifest required, and where it lives

Manifest 06's visual half is void; its behavioural half is not. Each of these is
a bug someone already paid for.

| Rule | Where |
|---|---|
| INV-1 / INV-6 — scroll anchored to content, clamped on shrink | `HistoryScreen.list` — `.scrollPosition(id:anchor:.top)` |
| INV-2 / INV-3 — identical data produces no new list; every mutation clears the signature | `HistoryStore.apply`, `ClipItem.historySignature` |
| INV-5 — rows reserve their full height, never a character-count estimate | `HistoryRow.reservedHeight` |
| INV-8 / INV-9 — rows are containers, not options; selection is announced | `HistoryRow` (`children: .contain`), `AccessibilityNotification.Announcement` |
| **INV-10 — sensitive content never reaches the view or the a11y tree** | `ClipItem.init` — the plaintext is *discarded*, not masked |
| INV-12 — no raw error text, ever | `DaemonError`, `PathRedactor` |
| INV-24 — a failed shortcut registration does not crash startup | `GlobalHotkey` |
| INV-25 — dismissing hands focus to the *previous* app | `StatusItemController.popoverDidClose` |
| INV-26 — copy-then-hide, never hide-then-copy | `HistoryStore.copy` returns success; `HistoryScreen.copy` hides only then |
| INV-27 — polling is visibility-gated | `HistoryStore.start`/`stop`, `DevicesStore.start`/`stop` |
| INV-29 / INV-30 — optimistic writes revert the specific field; busy flags always release | `HistoryStore.togglePin`, `defer` in `DevicesStore` |
| INV-32 / INV-33 — selection by id; late replies never clobber newer ones | `HistoryStore.selectedID`, `refreshSequence` |
| INV-34 — only `not_ready` is retried | `DaemonClient.send` |
| INV-35 — windows excluded from screen capture by default | `UtilityWindow`, `StatusItemController.applyContentProtection` |
| INV-36 — closing a window hides it; only Quit exits | `UtilityWindow.windowShouldClose` |
| INV-37 — blocking IPC never on the main thread | `DaemonClient`'s private dispatch queue |
| §3.1.8 — deferred delete with a 5 s undo, committed on close | `PendingDeletes` |
| §3.1.11 / §3.2.5 — five distinct empty states, "clipboard service" vocabulary | `StateView`, `LoadPhase` |
| §5.1 — 3 s / 5 s / 10 s / 30 s cadences | `LoadPhase.pollInterval`, `DevicesStore` |
| A11Y-11 — reduced motion | `@Environment(\.accessibilityReduceMotion)` in `HistoryRow`, `HistoryScreen` |

### Where the manifest was followed by not following it

Four places where a v1 rule maps onto something that no longer exists. Each is a
judgement, stated so it can be overruled:

- **Sensitive content is dropped, not blurred.** v1 blurred with click-to-reveal
  and needed INV-11 (re-hide on blur, re-hide after 10 s idle) to make that
  safe. v2 has no reveal at all, so the plaintext never leaves the service —
  strictly stronger, and INV-11 has nothing left to govern.
- **No optimistic copy-to-top (INV-31).** v1's service restamped an item on
  copy; v2's does not — `copy` reads the row and writes the pasteboard, and the
  capture loop decides what happens next. Moving the row locally would be this
  app inventing an order the service does not have.
- **No QR/SAS pairing flow (INV-13/14/15, §3.3).** v2's pairing is a code plus
  an address over the `PairCreate`/`PairAccept` methods; there is no PAKE
  session, no SAS digits and no TTL to count down. The *reasons* behind those
  rules were carried across instead: the code is hidden until asked for, is
  never put on the pasteboard (this is a clipboard manager — copying it would
  file the pairing secret into the user's own history and then sync it), is
  never logged, and is dropped when the sheet closes.
- **No Quick-Paste popup as a second window (§3.5).** The popover *is* that
  surface here. Its keyboard rules follow the popup's (type-to-filter with the
  field always focused, arrows forwarded to the list) rather than the main
  window's.

---

## Dependencies

One, and it is stated rather than hidden (CLAUDE.md rule 1):

**[`KeyboardShortcuts`](https://github.com/sindresorhus/KeyboardShortcuts)** —
global hotkey registration plus the recorder control. Hand-rolling it means
Carbon `RegisterEventHotKey` glue *and* a key-capture view, and the capture is
where the subtlety lives: manifest 06 INV-23 requires the binding to come from
the *physical* key so a Dvorak or AZERTY layout records the same combination,
and A11Y-13 requires the control to announce the bound accelerator rather than
its glyphs. Writing that again is the "it's only a few lines" trap rule 1 names.

Tradeoff: one third-party package, MIT, no transitive dependencies, macOS-only
(fine — so is this target). Blast radius is contained to `GlobalHotkey.swift`,
the only file that imports it.

Everything else is the platform: `SMAppService` for login items, `Network`-free
POSIX sockets for IPC, `Observation` for state.

---

## Building

```sh
cd apps/macos
swift build          # the executable
swift test           # CopyPasteKit's tests
make app             # assembles .build/CopyPaste.app  (never run — see above)
```

`swift build` alone is enough to run the app, but two things need a real bundle:
`LSUIElement` (no Dock icon) and `SMAppService` (open at login). That is what
`make app` is for. Opening `Package.swift` in Xcode gives a buildable scheme for
both targets.

The app looks for `copypaste-daemon` in its own bundle first, then
`/opt/homebrew/bin`, `/usr/local/bin`, `~/.cargo/bin`. `make app` copies
`target/release/copypaste-daemon` in when it exists — a shipped app must not
silently drive whichever build happens to be on `PATH`.

---

## Known gaps

- **The service is spawned as a child process**, so quitting CopyPaste takes it
  down. A `LaunchAgent` is the right home for it and is not written.
- **No pagination.** The popover loads one page of 200. The service holds more,
  and the footer says so ("50 of 214"), but there is no load-more.
- **No app icon**, no `.icns`. The menu-bar item uses an SF Symbol template
  image, which is the part that matters.
- **Sync results are per-run and not persisted**, so closing the Devices window
  loses the last run's counts.
- **No localisation.** Strings are inline English.
