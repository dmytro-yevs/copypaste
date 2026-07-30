# CopyPaste

Encrypted clipboard manager for macOS and Android.

**Status: v2.0.0-alpha.1 — a working core, on a rewrite branch.** The daemon
captures, encrypts, stores and searches; the CLI drives all of it over a local
socket; two devices pair and sync over a Noise channel. What is *not* yet
verified is listed below, in detail, because most of it is unverifiable on the
Linux host this is developed on.

**One cross-platform app: Tauri v2 + React, for macOS and Android both.**
`crates/copypaste-ui` is the product surface, not a placeholder — see
[ADR-0002](docs/adr/0002-one-cross-platform-app.md), which reverses an earlier
decision to write a SwiftUI app and a Compose app. v1's visual design is not
being carried over: the new look is undecided, so the design tokens in
`design/` still hold v1's values and those values are placeholders, not the
target.

---

## Why the rewrite

`main` was reset to an empty history on 2026-07-29. The previous
implementation is preserved in full at **`archive/v0.4.1-pre-rewrite`** (2,153
commits) — nothing was lost, it is one `git switch` away.

An audit of all eight subsystems of v0.4.1 found ~150k lines of Rust and ~22k of
TypeScript for a clipboard manager, with the same problems solved repeatedly:
six independent retry/backoff implementations, three rate limiters, three models
of one wire contract, two hand-written ASN.1 parsers, two regex secret engines.
Alongside that, several subsystems were dead — a complete HELLO/HAVE/WANT sync
engine (~5k lines including tests) that the daemon never instantiated, and a
telemetry crate whose own documentation stated it was never wired to a caller.

The cause was a "prefer hand-rolling" norm that had outlived the document
defining it. v2 inverts that default: **a dependency is the default, and
hand-rolling needs a written reason.** See [CLAUDE.md](CLAUDE.md).

Current size, for comparison: ~25k lines of Rust across seven crates and ~2k of
TypeScript, with roughly 520 Rust tests and 10 frontend tests.

## Status

Three columns, and the middle one is the honest part. "Unverified" does not mean
"probably fine" — it means written against the manifests, compiled where the
host allows, and never observed doing its job.

### Works — exercised by tests, and by the demo scripts end to end

| Area | Crate | Notes |
|---|---|---|
| Crypto | `copypaste-core` | XChaCha20-Poly1305 + HKDF-SHA256, item id bound as AAD, fail-closed on a wrong key or AAD, zeroization |
| Storage | `copypaste-core` | One SQLCipher schema (no migration ladder), r2d2 pool, FTS5 search, tombstones, pins, keyset pagination, cap eviction |
| Secret detection | `copypaste-core` | Ruleset sourced from gitleaks, NFKC normalisation, Luhn validation, confidence model; sensitive items never reach the index |
| Capture pipeline | `copypaste-daemon` | Clipboard behind a trait: `changeCount` change detection, burst handling, self-write suppression and the `org.nspasteboard.*` opt-outs are tested against the fake source on any host |
| IPC | `copypaste-ipc`, `copypaste-daemon` | `0600` Unix socket, newline-JSON, `LinesCodec` framing, one typed contract crate shared by daemon, CLI and UI bridge |
| CLI | `copypaste-cli` | `list search add copy delete clear pin unpin status pair peers unpair sync`; `--json` for scripting |
| Peer sync | `copypaste-p2p`, `copypaste-daemon` | Noise `NNpsk0` over TCP, pairing codes, LWW merge, delete-wins, sensitive items never leave their origin device |
| Design-token pipeline | `design/` | One DTCG source compiled by Style Dictionary per platform. The pipeline works; the *values* in it are v1's and are placeholders (see below) |
| Interim history window | `crates/copypaste-ui` | React 19 + Tailwind v4 + React Query + TanStack Virtual; `npm run build` and `npm test` pass, and the Tauri shell builds and launches on Linux. Temporary — see the note at the top |

`scripts/demo.sh` drives the built binaries through capture → encrypt → store →
search → paste-back, and asserts the security rules. `scripts/demo-p2p.sh`
stands up two daemons with separate data directories and ports and asserts
pairing, two-way convergence, a no-op second sync, refusal of a wrong code, and
unpairing. Both pass.

### Unverified — written, but never observed working

| Thing | Why not |
|---|---|
| macOS NSPasteboard backend | We develop on Linux. `clipboard::macos` has never been compiled or run. The shared change-detection state machine it uses *is* tested; the binding-level assumptions are not. The daemon reports which backend is live (`status`), so a demo cannot be mistaken for the real thing. |
| macOS Keychain device-secret store | Same reason. It is behind the `macos-keychain` cargo feature; on Linux the daemon falls back to a `0600` file store, which is a development posture and not a shipping one. |
| The UI as a *view* | WebKitGTK executes no JavaScript under headless Xvfb without a GPU. Confirmed rather than assumed: a stub daemon on the socket saw zero requests from the launched app, while the same probe against the CLI proved the harness worked. The shell is verified; what it renders is covered only by jsdom unit tests. |
| mDNS discovery | The container has no multicast. Discovery is a convenience only — an explicit `--addr` always works, and that is the path the demo and the tests take. |
| `copypaste-cloud` against a live Supabase project | Every test runs against in-process fakes and mocked HTTP. Nothing has ever spoken to a real deployment. It is also **not wired into the daemon or the CLI** — the crate compiles and is tested, and nothing calls it yet. |

### Not built

**The Android target** of the Tauri app, which needs the bridge to embed the
core in-process rather than speak to a daemon (ADR-0002), and **the new visual
design**. Also: image, file and rich-text capture (text only today) ·
frontmost-app attribution, private mode, the app-exclusion list · cloud sync
wired to the daemon, with its quota/TTL job and signed LWW metadata ·
age-based retention (`evict_older_than` exists, no loop calls it) · rate
limiting (`governor` is declared in the workspace manifest and unused) ·
release packaging — the Homebrew tap and its cask, per
[ADR-0001](docs/adr/0001-macos-distribution-without-a-developer-id.md) ·
telemetry.

## What is here

| Path | What it is |
|---|---|
| `crates/copypaste-core` | Crypto, storage, secret detection. No async, no IO beyond SQLite and the key store. |
| `crates/copypaste-daemon` | Clipboard capture, the IPC server, the peer listener. The only crate that holds a key and a database at the same time. |
| `crates/copypaste-cli` | `copypaste`. Speaks IPC and nothing else — it cannot open the database or decide what is sensitive. |
| `crates/copypaste-ipc` | The one model of the wire contract, plus the path redactor every client shares. |
| `crates/copypaste-p2p` | Noise `NNpsk0` transport, pairing, the LWW merge, mDNS discovery. |
| `crates/copypaste-cloud` | Supabase auth, PostgREST, Realtime, client-side encryption, the sync driver. Not yet wired in. |
| `crates/copypaste-ui` | The app: Tauri v2 + React 19, and the bridge to the daemon socket. macOS and Android both. |
| `design/` | The Style Dictionary pipeline. One token source compiled per target; its current values are v1's and are placeholders. |
| `scripts/` | `demo.sh` and `demo-p2p.sh`. |
| `docs/rewrite/port-manifest/` | ~9,100 lines of specification harvested from v1 and its tests: ~500 acceptance tests, 200+ recovered bug IDs. **The behaviour in them is the requirements.** |
| `docs/rewrite/target-architecture.md` | The library-first stack, per subsystem, the things that stay custom on purpose, and the three decisions taken since it was written. |
| `docs/rewrite/design-reference.html` | Visual reference for the **v1** UI. Historical: v1's design is not being carried over. |
| `docs/adr/` | Decisions with consequences that outlive the commit that took them. |
| `CLAUDE.md` | The working rules. |

`compat/` no longer exists. It held the evidence that v2 could still open v1
data; it was removed when backward compatibility was dropped (`e148e3c1`). The
fixtures remain reachable on the pre-rewrite branches.

## No upgrade path

v2 does not read data written by v0.4.x. Existing installs lose their clipboard
history and their paired devices; devices must be paired again.

This is deliberate. It removes the migration ladder, `key_version` dispatch, the
rotation and repair sweeps, and every wart kept only for bug-compatibility — a
large share of the complexity this rewrite exists to shed. v2 stores its history
in `copypaste-v2.db`, a distinct filename, so an old file is never opened or
modified and remains on disk if you downgrade.

## Building

Rust 1.96 (see `rust-toolchain.toml`). SQLCipher is built from source by
`rusqlite`'s `bundled-sqlcipher` feature, so the first build is slow and needs a
C toolchain.

```sh
cargo build --release -p copypaste-daemon -p copypaste-cli
cargo test --workspace

./scripts/demo.sh        # capture → encrypt → store → search → paste back
./scripts/demo-p2p.sh    # two daemons, pairing, two-way convergence
```

The daemon does not fork; backgrounding is the service manager's job.

```sh
target/release/copypaste-daemon --foreground &
target/release/copypaste list
```

On Linux the clipboard source is a fake, drivable from `COPYPASTE_FAKE_CLIPBOARD`
or over IPC, so the pipeline is demonstrable off a mac. `copypaste status`
always names the live backend.

The interim window, for as long as it exists:

```sh
cd crates/copypaste-ui
npm install
npm run build && npm test
npm run tauri dev        # needs a GPU-backed display; see "Unverified" above
```

## The manifests

| Manifest | Subject |
|---|---|
| 01 | Clipboard capture — NSPasteboard quirks, privacy markers, 39 invariants |
| 02 | Crypto — key derivation, AAD binding, fail-closed semantics |
| 03 | Storage — schema, retention, the sensitive-never-in-FTS rule, SQLCipher parameters |
| 04 | IPC — the method catalogue and the error-code taxonomy |
| 05 | Sync — merge ordering, and the relay→Supabase parity checklist |
| 06 | UI — behaviour contract and accessibility (binding), v1's design tokens (reference) |
| 07 | Secret detection — the full ruleset with confidence thresholds |

[`docs/rewrite/port-manifest/README.md`](docs/rewrite/port-manifest/README.md)
records, per manifest, which sections are binding requirements and which became
reference material — dropping backward compatibility retired the *formats*, and
rejecting v1's design retired the *visuals*. Read it before treating any
manifest section as a requirement. What neither decision touched is the
behaviour: several hundred acceptance tests encoding bugs someone already paid
for.

## Licence

MIT or Apache-2.0, at your option.
