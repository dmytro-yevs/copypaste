# ADR-0003 — One command surface, two backends

**Status:** accepted · 2026-07-30 · **implements the seam ADR-0002 creates**
**Scope:** how the Tauri bridge in `crates/copypaste-ui/src-tauri` talks to the
rest of the system on the desktop and on Android, including platform boundaries
that remain subject to native qualification.

## Decision

The bridge declares **one trait**, `backend::Backend`, holding every operation
the product supports, typed on `copypaste_ipc`. Two implementations satisfy it:

| target | implementation | how it works |
|---|---|---|
| macOS / Windows desktop | `backend::daemon::DaemonBackend` | newline-delimited JSON over the platform local-IPC endpoint |
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

The Rust-owned Tauri command registry maps the shared backend operations to
generated TypeScript names and signatures. The CLI and app therefore exercise
the same IPC methods where a daemon is present; Android calls the embedded
implementation through the same trait.

| command | `Method` |
|---|---|
| `list`, `search`, `add_item`, `copy_item`, `delete_item`, `delete_all`, `set_pinned`, `status` | `List`, `Search`, `Add`, `Copy`, `Delete`, `DeleteAll`, `Pin`, `Status` |
| `pair_create_invite`, `pair_progress`, `pair_confirm`, `pair_cancel`, `peers`, `unpair`, `revoke`, `sync_now` | `PairCreateInvite`, `PairProgress`, `PairConfirm`, `PairCancel`, `Peers`, `Unpair`, `Revoke`, `SyncNow` |
| `reveal_item` | `Get` |

`start_service` and `restart_service` call the app-owned
`service::Supervisor`. Restart requests shutdown through the local IPC contract
even for an adopted daemon, waits for it to stop, then starts the bundled
version; only an owned child is eligible for the forced-kill fallback. This
preserves ADR-0004's ownership boundary. Their source-level presence is not
native lifecycle qualification.

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

### `reveal_item` is a side-effect-free read

`reveal_item` needs a **side-effect-free read of one item**, because its caller
is a user asking to *look* at a secret. It uses `copypaste_ipc::Method::Get`,
which the daemon dispatcher and both backend implementations route to an item
read without writing the system pasteboard.

Routing reveal through `Copy` was the first shape of this and it is wrong:
looking at a password would silently publish it to every app on the machine
that reads the pasteboard — and raise macOS 16's paste alert — as a side effect
of a gesture that promised only to show it. Reveal and copy are two buttons on
the row because they are two intentions.

Paging `Method::List` to find the id remains a non-option: it would scan the
history, break past the server's page clamp, and pull secrets into a response
nobody asked for. The `Get` route keeps reveal and copy as distinct intentions.

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

## Embedded operation parity

The embedded adapter routes `set_config`, `backup`, `restore`, and
`reorder_pinned` to its persisted settings, backup, and store owners. This
keeps destructive backup/restore and ordering logic behind the shared backend
surface rather than adding Android-only approximations. The source routes do
not by themselves qualify live Android settings behaviour on a device.

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

### Android capture boundary

Android capture is implemented through the app's capture bridge and queue, with
Rust draining accepted clips into the embedded backend. Platform restrictions
remain part of the capture contract; source presence is not a claim that a
particular Android device or release artifact has been qualified.

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

### Dependency boundary

The Android Keystore adapter uses the platform crypto path to wrap the device
secret rather than introducing another crypto stack. Dependency versions and
their maintenance status are owned by `Cargo.toml`; this ADR retains only the
security decision.

## Verification status

This ADR records the boundary and its security consequences, not a rolling
test-count inventory. Required platform evidence is owned by the testing policy
and release qualification; source-level compilation or host tests never stand
in for native Android or desktop evidence.
