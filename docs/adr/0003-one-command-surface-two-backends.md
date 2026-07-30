# ADR-0003 — One command surface, two backends

**Status:** accepted · 2026-07-30 · **implements the seam ADR-0002 creates**
**Scope:** how the Tauri bridge in `crates/copypaste-ui/src-tauri` talks to the
rest of the system on macOS and on Android, and what still blocks the Android
half.

## Decision

The bridge declares **one trait**, `backend::Backend`, holding every operation
the product supports, typed on `copypaste_ipc`. Two implementations satisfy it:

| target | implementation | how it works |
|---|---|---|
| macOS / desktop | `backend::daemon::DaemonBackend` | newline-delimited JSON over the daemon's `0600` Unix socket |
| Android | `backend::embedded::EmbeddedBackend` | `copypaste-core` and `copypaste-p2p` in the app process |

The choice is a compile-time type alias, `backend::SelectedBackend`. There is no
dynamic dispatch and no runtime probe.

**`crates/copypaste-ui/src-tauri/src/commands/` contains no `cfg` at all.** The
`#[tauri::command]` functions are written against the alias, so the command
names, argument names and return types are the same text on both platforms.
That is the identity ADR-0002 calls a hard requirement, and it is now enforced
by the compiler rather than by review.

## Why a trait rather than two `cfg` modules

Two `cfg` modules with matching function names compile happily while
disagreeing about argument order, optionality or return type, because the
compiler only ever sees one of them. The trait makes both answer to one
declaration.

This is not hypothetical. Type-checking the Android backend on a Linux host —
possible only because of the `embedded-backend` feature described below — caught
`Peer::last_addr` being a `SocketAddr` where `PeerInfo::last_addr` is a
`String`. Under `cfg`-only separation that error would have waited for the first
machine with an Android NDK.

## The command surface

Thirteen backend operations, plus `get`, chosen to match the CLI's verb set —
because an operation the CLI can reach and the app cannot is a feature with no
UI (CLAUDE.md rule 6), and pairing is the case that rule was written from.

Fourteen Tauri commands sit on top, named after `copypaste_ipc::Method` rather
than as English verbs — `delete_all` for `DeleteAll`, `set_pinned` for
`Pin { pinned }`, `sync_now` for `SyncNow`. The wire enum is the single model of
the contract, so a name that matches it is one fewer mapping to keep straight,
and `crates/copypaste-ui/src/lib/ipc.ts` was already written against these
names.

| command | `Method` |
|---|---|
| `list`, `search`, `add_item`, `copy_item`, `delete_item`, `delete_all`, `set_pinned`, `status` | `List`, `Search`, `Add`, `Copy`, `Delete`, `DeleteAll`, `Pin`, `Status` |
| `pair_create`, `pair_accept`, `peers`, `unpair`, `sync_now` | `PairCreate`, `PairAccept`, `Peers`, `Unpair`, `SyncNow` |
| `reveal_item` | none — see below |

**`start_service` is deliberately not implemented.** `lib/ipc.ts` declares it so
the offline screen can offer to start the daemon rather than only telling the
user to open a terminal, and that is a reasonable thing to want. It is not
implemented here because it is a product decision that touches ADR-0001: the
daemon ships as a separate Homebrew formula and manages its own lifecycle, and
having the app spawn it introduces a `PATH` assumption, an orphan-process
question, and an error path whose natural text names a filesystem path. The
frontend's `unavailable` fallback is the correct behaviour until that decision
is taken, and taking it should amend ADR-0001 rather than happen in a bridge
commit.

## Sensitive content is discarded at the process boundary

`copypaste_ipc::Item` carries plaintext. That is right on the wire — the daemon
decrypts on the way out and the socket is `0600` — and wrong in a WebView, where
a string is reachable by every component, the accessibility tree, devtools, a
heap snapshot, and anything that serialises a props object into a log.

Manifest 06 INV-10 requires sensitive content to be **absent** from the view
rather than obscured over the top of it. A rule ("remember to blank `content`
when `is_sensitive`") applied at each of nine commands is exactly the shape of
defect the manifest records v1 shipping.

So `model::UiItem` has private fields and no public constructor. The only way to
make one is `From<Item>`, which is total and drops the plaintext on the spot,
and every command signature returns `UiItem`. `content` is `Option<String>`, so
"there is no content" is a state the frontend's type checker sees rather than a
value it must test for. A new command cannot return the wire type without
changing its own signature to say so.

**This changes the frontend contract**, and `crates/copypaste-ui/src/lib/ipc.ts`
has to follow: `Item.content` becomes `string | null`, and a sensitive item's
text arrives only from the new `reveal_item(id)` command, which exists for the
explicit reveal gesture and is the single route back to a secret.

Concretely, `components/HistoryRow.tsx` renders `previewOf(item.content)` once a
sensitive row is revealed (`masked = is_sensitive && !revealed`). That path now
receives `null` and must call `reveal_item(id)` instead. Every *other* use of
`content` in the frontend is already guarded by `is_sensitive`, so nothing else
breaks — and that near-miss is the argument for the boundary: the guard was
correct in five places and absent in the sixth.

Everything else — copy, pin, delete — travels by id and does its work in the
backend, so the plaintext never needs to be in the WebView in order to be
*used*. That is also why the `clipboard-manager:allow-write-text` capability was
dropped: the WebView no longer writes the clipboard, so granting it would be a
capability the app does not need.

### `reveal_item` does not work on desktop yet, on purpose

`reveal_item` needs a **side-effect-free read of one item**, because its caller
is a user asking to *look* at a secret. `copypaste_ipc::Method` has no such
verb. The nearest is `Copy`, which puts the content on the system pasteboard.

Routing reveal through `Copy` was the first shape of this and it is wrong:
looking at a password would silently publish it to every app on the machine
that reads the pasteboard — and raise macOS 16's paste alert — as a side effect
of a gesture that promised only to show it. Reveal and copy are two buttons on
the row because they are two intentions.

Paging `Method::List` to find the id is the other non-option: `List` does return
sensitive plaintext, but the scan is O(history), breaks past the server's
1 000-row clamp, and pulls secrets into a response nobody asked for.

So `DaemonBackend::get` returns `Unsupported`. The reveal button is visibly
broken on macOS rather than invisibly unsafe, and a test pins the behaviour —
the regression it guards is silent, because a `Copy`-backed `get` returns the
right content and would pass any test that only checked the value.

**The fix is small and lives elsewhere:** add `Method::Get { id }` to
`copypaste-ipc` and an arm to the daemon's dispatcher. Two crates this change
does not own. The in-process Android backend already implements `get` properly,
because there it is just a store read.

## Dependencies taken (CLAUDE.md rule 1)

Checked before writing, not after. No native code is written in this crate.

| need | crate | tradeoff stated |
|---|---|---|
| menu-bar item | `tauri`'s own `tray-icon` feature | already a dependency |
| popover show/hide/focus | `tauri`'s window API | already a dependency |
| global hotkey | `tauri-plugin-global-shortcut` → `global-hotkey` | pulls `keyboard-types`, `xkeysym`; desktop-only, and the crate is itself `cfg`'d off on Android |
| launch at login | `tauri-plugin-autostart` → `auto-launch` | writes a `LaunchAgent` plist; the alternative is hand-writing that plist, its removal path, and a second mechanism per platform |

No exemption from rule 1 is claimed. Nothing here is hand-rolled.

One thing the plugins do not provide is a *policy*:
`shell::hotkey::is_permission_free`, which refuses the five media keys that
would cost an Accessibility grant. That is a constraint from ADR-0001 that no
upstream crate knows about, not a reimplementation of anything.

`MacosLauncher::LaunchAgent` rather than `AppleScript` is deliberate: the
AppleScript strategy drives System Events to add a Login Item, which is an
Automation TCC grant held against a cdhash that changes on every ad-hoc-signed
build — revoked on every update, for a setting the user made once.

## What the Android backend cannot do yet, and why it refuses rather than guesses

Four operations — `add`, `pair_create`, `pair_accept`, `sync` — return a typed
`Unsupported` error instead of an implementation. **This is a refusal, not an
oversight.**

`copypaste-daemon` is a binary crate with no `[lib]` target, so nothing in it is
importable. The two things the Android backend needs are both inside it:

**1. The ingest pipeline** — `daemon::capture::ingest`. Trim, hash,
dedup-probe, detect, choose the id *before* the seal because the AEAD binds it,
encrypt, insert, record origin, evict. Every step is a decision with a bug
behind it: manifest 01 I-33 (a dedup-probe failure must fall through to the
insert), the `cutoff_ms` argument that is an absolute epoch stamp and not a
window width, the write-time half of "sensitive items never reach the search
index".

Re-typing those forty lines into the UI crate would create a second ingest path.
`capture.rs`'s own module docs record what happened last time there were two:
*"v1 had two ingest paths that drifted: the IPC one forgot the dedup probe, so
`copypaste add` could insert a row the poll loop would have collapsed."*
CLAUDE.md rule 1 names "it's only a few lines" as the failure mode by name, so
the code says no instead.

**2. The p2p node** — a TCP listener holding the pre-shared keys, mDNS
discovery, and the sync-metadata connection `daemon::p2p::meta` opens onto the
same SQLCipher file for the columns `StoredItem` does not carry.
`copypaste-p2p` provides the transport, the merge and the protocol; it does not
provide the node that owns them.

### The fix, in crates this change does not own

* **Lift `capture::ingest` down into `copypaste-core`.** Its only dependencies
  are core's own three modules — crypto, storage, sensitive — so it arguably
  belongs there already and its presence in the daemon is a layering accident.
  The daemon keeps the poll loop; the core gains `ingest`.
* **Lift the p2p node up into `copypaste-p2p`**, as a type owning the listener,
  discovery and the metadata connection, with the daemon holding one.

Both are behaviour-preserving moves. Until they happen the Android build can
read, copy, pin, delete and clear history, and list and forget peers — but it
cannot add an item or sync, which means it is not shippable.

### Also outstanding on Android

* **No Android Keystore backend.** `copypaste_core::Keyring::load_or_create`
  falls through to the `0600`-file store, whose own docs call it a development
  posture and say "Android must use the Android Keystore before shipping".
* **No clipboard capture.** Android has no equivalent of the daemon's poll loop
  and reading the clipboard in the background is restricted from Android 10 on.
  What items enter history and how is an undecided product question, not just
  missing code.
* **Window positioning under the menu-bar item** is now implemented —
  `shell::window::anchor`, with the clamp and the scale-factor arithmetic, and
  five tests over the cases that have a right answer (right-hand edge, a
  monitor with a negative origin, a Retina scale factor, a window larger than
  the screen). What cannot be tested here is everything around it: the tray
  rect and the monitor list come from the platform, and this host has neither.
  Confirming the popover actually lands under the icon needs a Mac.

## Verification status

Stated plainly because it matters. This work happened on a Linux host with no
macOS SDK and no Android SDK.

| | compiles | tests run | run on the platform |
|---|---|---|---|
| desktop backend, commands, model | yes | yes, 33 tests | **no** |
| shell (tray, window, hotkey, autostart) | yes | the hotkey guard, yes | **no** |
| embedded backend | yes, under `--features embedded-backend` on Linux | yes | **no** |
| Android build config | JSON validated against the `tauri-utils` schema | n/a | **no — `cargo tauri android` has never been run here** |

The `embedded-backend` cargo feature exists precisely so the Android path is
type-checked *somewhere*. Without it the whole in-process backend would be dead
text on every machine the project is actually built on — which is how the
deleted Compose app reached ~2,500 uncompiled lines, and the mistake ADR-0002
reversed a decision to avoid.

## What would change this

The daemon growing a `[lib]` target, or the two extractions above landing —
either removes the reason the Android backend refuses four operations. If
instead Android is descoped, the `Backend` trait and the `embedded-backend`
feature should go with it rather than being left as scaffolding.
