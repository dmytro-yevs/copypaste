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
UI (AGENTS.md rule 6), and pairing is the case that rule was written from.

The Rust-owned Tauri command registry sits on top and generates the TypeScript
names and signatures. Daemon-backed commands map to `copypaste_ipc::Method`;
native-only commands stay explicit in the same registry.

| command | `Method` |
|---|---|
| `list`, `search`, `add_item`, `copy_item`, `delete_item`, `delete_all`, `set_pinned`, `status` | `List`, `Search`, `Add`, `Copy`, `Delete`, `DeleteAll`, `Pin`, `Status` |
| `pair_create_invite`, `pair_progress`, `pair_confirm`, `pair_cancel`, `peers`, `unpair`, `revoke`, `sync_now` | `PairCreateInvite`, `PairProgress`, `PairConfirm`, `PairCancel`, `Peers`, `Unpair`, `Revoke`, `SyncNow` |
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
rather than obscured over the top of it. A per-command "remember to blank
`content`" rule cannot uphold that boundary.

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

## Dependencies taken (AGENTS.md rule 1)

Checked before writing, not after. One JNI entry point is written here, and
nothing else native — see the device secret below for why that one exists.

| need | crate | tradeoff stated |
|---|---|---|
| menu-bar item | `tauri`'s own `tray-icon` feature | already a dependency |
| popover show/hide/focus | `tauri`'s window API | already a dependency |
| global hotkey | `tauri-plugin-global-shortcut` → `global-hotkey` | pulls `keyboard-types`, `xkeysym`; desktop-only, and the crate is itself `cfg`'d off on Android |
| launch at login | `tauri-plugin-autostart` → `auto-launch` | writes a `LaunchAgent` plist; the alternative is hand-writing that plist, its removal path, and a second mechanism per platform |
| the app context, for the keystore below | `jni`, `ndk-context` | Android-only, and both already in the tree under tao; costs this crate its `forbid(unsafe_code)` |

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

Four operations — `set_config`, `backup`, `restore` and `reorder_pinned` —
return a typed `Unsupported` error instead of an implementation. **This is a
refusal, not an oversight.** `copypaste-daemon` is a binary crate with no
`[lib]` target, so the logic behind `backup` and `restore` still lives somewhere
Android cannot import: `server::dbadmin` owns the validate-then-swap that keeps
a bad backup from replacing a working history. Approximating it is the second
implementation AGENTS.md rule 1 exists to stop, and a file copy that looks like
a restore and is not would be data loss. `set_config` refuses because there is
nowhere to put the answer: a setting that does not outlive the process is not a
setting.

`reorder_pinned` refuses on both platforms — `Store::reorder_pinned` exists, but
this build has no `Backend` route to it yet (parity finding 19).

### Shared ownership behind available operations

Add, pairing, sync, export, and import are available because their product
logic lives below the backend boundary rather than in either adapter.

* **`add`** — `capture::ingest` moved down into `copypaste_core::ingest`. One
  implementation, four callers.
* **`export`, `import`** — `server::transfer` moved down into
  `copypaste_core::transfer`. The two rules that had to survive the move are the
  reason it was a move and not a reimplementation: an export *counts* what it
  withheld, and an import re-runs the detector over every item so an edited file
  cannot mark a credential clean (manifest 04, PG-26). The daemon's handler is
  now only the wire mapping — a pathless `Response`, and waking the two sync
  transports once something was written.
* **Pairing, sync, and both discovery operations** — the peer node moved up into
  `copypaste_p2p::node`, generic over `SyncSource`, and then `SyncSource` itself
  moved down into `copypaste_core::sync::StoreSource`.
  The second half was the one that mattered: a node generic over a trait with
  exactly one implementation, and that implementation built on two
  daemon-private modules, is still a daemon-only node.

Making `StoreSource` a core type meant closing the gap it was written around —
`StoredItem` now carries `content_hash`, `deleted` and `origin_device_id`, and
`Store` answers `summaries`, `versions`, `versions_since` and `upsert`. The
daemon's second SQLCipher connection is gone with it. **`copypaste-core`
therefore depends on `copypaste-p2p`**, for the wire types and for
`merge_decision` — the one comparator both transports apply (manifest 05
INV-C2). The edge runs that way because `copypaste-p2p` deliberately knows
nothing about a database; forking the comparator to keep the two crates apart is
the defect INV-C2 records. It adds no third-party weight to a shipped binary,
because every consumer of the core already links `copypaste-p2p`.

### Also outstanding on Android

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

## The device secret on Android

`copypaste_core::Keyring::load_or_create` used to fall through to the
`0600`-file store on Android — a development posture, shipped. It no longer
can: `crypto::keystore` now selects a third backend on `target_os = "android"`,
and the file backend is not compiled on that target at all, so no
misconfiguration lands on it.

**The secret is wrapped, not stored.** The Android Keystore holds keys, and a
hardware-backed key is non-exportable, so "put 32 bytes in the Keystore" is not
available. An AES-GCM key lives in the Keystore under a frozen alias and never
leaves it; the device secret is sealed with that key and kept in app-private
`SharedPreferences`. Compromise of the file yields ciphertext.

**`setUserAuthenticationRequired(false)`.** A clipboard that receives items in
the background must decrypt while the screen is locked; with it true,
`Cipher.init` throws `UserNotAuthenticatedException` exactly when an item
arrives. The consequence accepted: a device unlocked by an attacker can
decrypt. Because the key is not auth-bound, biometric re-enrolment does not
invalidate it — that applies only to `setInvalidatedByBiometricEnrollment`.

**No StrongBox.** `setIsStrongBoxBacked(true)` throws
`StrongBoxUnavailableException` on devices without the chip, so it needs a
per-device fallback, and the fallback is the TEE-backed key already in hand. A
secure element also has a small key budget and is markedly slower.

**An invalidated key is not authorisation to mint.** A wrapping key that no
longer opens its own blob surfaces as `CryptoError::KeystoreEntryUnusable` and
stops there (port manifest 02, I-20). The history was encrypted under a secret
that can no longer be read; a fresh one would turn that into what looks like
corruption. The honest report is that the history is gone and clearing app data
starts over. Clearing app data and uninstalling remove the Keystore alias and
the database together, so the ordinary destructive paths stay consistent.

### The secret follows the data directory

`Keyring::load_or_create` now takes the directory holding the history, because
security review F-11 is that `--data-dir` moved the database and left the
secret behind. The keystore backends are user- or app-scoped and ignore the
argument; the file backend puts its file there instead of in
`directories::ProjectDirs`.

They all receive it anyway, because of the guard it enables: **a data directory
that holds a database but no secret is refused, not minted into.** "No entry"
otherwise means both "first run" and "we looked in the wrong place", and only
the first authorises a mint (I-20). The daemon and `EmbeddedBackend::open` each
pass the one directory they already derived, so there is no second derivation
to drift.

### The dependency, and what was evaluated (AGENTS.md rule 1)

`android-native-keyring-store` 1.0, with `keyring-core` 1.0. It is the design
above, already written: an `AndroidKeyStore` AES-GCM key wrapping values in
`SharedPreferences`, over JNI to Android's own crypto — no second crypto stack
(rule 1, exemption 3). Decisively, it reports a missing entry as
`Error::NoEntry`, distinct from `BadDataFormat`, `BadStoreFormat` and
`PlatformFailure`, which is precisely the classification I-20 turns on. It is
maintained by the `keyring-rs` authors; 1.0 is four months old.

Also evaluated: `keyring` 4 (the same store, plus a process-global default and
three desktop backends we do not want); `animo-secure-env` (last released
2024-07, and its iOS half duplicates `security-framework`);
`tauri-plugin-keystore` 2.1.0-alpha.1 (alpha, quiet since 2025-02);
`hardware-keystore` 0.0.1 (89 downloads). Hand-written JNI was the alternative
and is roughly 150 lines of `javax.crypto` calls that no machine in this
project compiles.

The tradeoff to state: it finds the JavaVM and app context through
`ndk-context`, which **Tauri does not populate** — tao keeps its own activity
registry and wry dropped the dependency. So the app supplies it:
`src-tauri/src/android_context.rs` is one JNI entry point, called from
`MainActivity.onCreate` before `super.onCreate` because Tauri's setup opens the
database during it. That is the only `unsafe` in `copypaste-ui`, and why its
crate attribute is now `deny` rather than `forbid`.

## Verification status

Stated plainly because it matters. This work happened on a Linux host with no
macOS SDK and no Android SDK.

| | compiles | tests run | run on the platform |
|---|---|---|---|
| desktop backend, commands, model | yes | yes, 33 tests | **no** |
| shell (tray, window, hotkey, autostart) | yes | the hotkey guard, yes | **no** |
| embedded backend | yes, under `--features embedded-backend` on Linux | yes | **no** |
| Android keystore backend | yes, under `--features android-keystore-typecheck` on Linux | none exist — they would need a device | **no** |
| the JNI context handover | yes, under `--features embedded-backend` on Linux | n/a | **no** |
| `KeystoreContext.kt`, its ProGuard rule, the `MainActivity` call | **no** | n/a | **no** |
| Android build config | JSON validated against the `tauri-utils` schema | n/a | **no** — `cargo tauri android` has never been run here |

Both new cargo features exist for the same reason: to compile Android code on a
machine that cannot build for Android. Neither selects anything — the backend
is chosen by `target_os` alone, because a feature is a way to ship without it
and that is what happened to the macOS Keychain.

What a first device run would falsify, in the order it would fail:

1. `System.loadLibrary("copypaste_ui_lib")` finds the library and
   `KeystoreContext.initialize` resolves to the Rust symbol — `UnsatisfiedLinkError`
   if either the name or the ProGuard rule is wrong.
2. `getSharedPreferences` and `KeyGenParameterSpec$Builder` succeed from a
   context captured before `super.onCreate`.
3. A second launch reads back the same secret rather than reporting
   `NoEntry` — which is the only observation that proves the round trip, and
   the one that would have caught a wrong store name.
4. The database opens under the derived key on that second launch.

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
