# CopyPaste

Encrypted clipboard manager for macOS and Android. One app — Tauri v2 + React —
on both platforms, over a shared Rust core. On desktop the app talks to a local
daemon; on Android it links the core in-process
([ADR-0003](docs/adr/0003-one-command-surface-two-backends.md)).

**v2.0.0-alpha.1, on the `v2-main` rewrite branch, unaudited.** `main` was reset
to an empty history on 2026-07-29; v0.4.1 remains intact at
`archive/v0.4.1-pre-rewrite` (2,153 commits). v2 reads nothing that version
wrote and uses a distinct database filename, `copypaste-v2.db`, so an old file
is never opened — [CLAUDE.md](CLAUDE.md) rule 3 has the reasoning and the one
obligation it creates.

The rewrite exists because v0.4.1 had grown to ~150k lines of Rust with six
retry implementations, three rate limiters and three models of one wire
contract. v2 inverts the norm that produced them: **a dependency is the default,
and hand-rolling needs a written reason.**

## Status

Three columns. The middle one is the point: *unverified* means written, compiled
and reviewed, and never observed doing its job on a platform we ship to.

### Works — covered by tests, and by the demo scripts where a script reaches

| Area | Notes |
|---|---|
| Crypto | XChaCha20-Poly1305 + HKDF-SHA256, item id bound as AAD, fail-closed, zeroized |
| Storage | One SQLCipher schema, r2d2 pool, FTS5 search, tombstones, pins, cap eviction |
| Secret detection | Ruleset sourced from gitleaks, NFKC normalisation, Luhn validation, confidence bands; a flagged item never reaches the index and never leaves the device |
| Capture | Clipboard behind a trait, so `changeCount` detection, burst handling, self-write suppression and the `org.nspasteboard.*` opt-outs are all tested against the fake source on any host |
| IPC | `0600` Unix socket, newline-JSON, `LinesCodec` framing; `copypaste-ipc` is the only model of the contract, shared by daemon, CLI and the Tauri bridge |
| CLI | `list search add copy get delete clear pin unpin status pair peers unpair sync cloud`, `--json` for scripting |
| Peer sync | Noise `NNpsk0` over TCP, pairing codes, LWW merge, delete-wins |
| Cloud sync | Supabase auth, PostgREST, Realtime, rows sealed client-side under an Argon2id key; wired to the daemon and the CLI — but see below |
| App | History, search, devices/pairing, settings; menu-bar item, popover, global hotkey and launch-at-login via Tauri plugins |
| Design tokens | One DTCG source compiled by Style Dictionary; shadcn/ui on Tailwind v4, zinc base in OKLCH, with contrast measured and gated (`design/README.md`) |

### Unverified

| Thing | What is and is not established |
|---|---|
| macOS `NSPasteboard` capture, macOS Keychain device-secret store | CI's `macos-check` job compiles and lints them on `macos-14` with `--all-features`, and runs the portable half of the suite there. Nothing drives a real pasteboard or a real keychain entry. On Linux the daemon falls back to a `0600` file store — a development posture, not a shipping one. |
| Cloud sync against Supabase | `scripts/demo-cloud.sh` drives two daemons through sign-in, convergence and sensitive-item refusal against a **local stub** (`scripts/cloud-stub.py`). Nothing has ever spoken to a real project, and no deployment has had `supabase/`'s schema and RLS policies applied. |
| The app as a rendered view | The `e2e/` suite drives the built app through `tauri-driver` → `WebKitWebDriver` under Xvfb, and WebKitGTK 2.52 does execute JavaScript and compute layout there. The suite is in flight, and WebKitGTK is neither WKWebView nor Android's WebView, so a green run is evidence about Linux only. |
| Packaging and release | `.github/workflows/release.yml`, `scripts/release/`, `Casks/` and `packaging/` are written from documentation and v1's scripts. No step of it — `codesign`, `hdiutil`, the Tauri macOS bundler — has run on a Mac. |
| mDNS discovery | This container has no multicast. Discovery is a convenience; an explicit `--addr` always works and is what the demo and the tests use. |

### Missing

A [parity audit](docs/rewrite/parity-audit.md) against v0.4.1 found nineteen
capabilities that were neither ported nor recorded as dropped. Pairing UI and
the popup/hotkey shell have since landed; the rest have not. In rough order of
what a user loses: no sensitive-item auto-wipe, no export/import, no
backup/restore, no device revocation or key rotation, dedup only inside a
60-second window (so re-copying an old item makes a second row), no daemon
config or server-owned settings, pairing codes that never expire, no streaming
updates, no discovery listing, no notifications, no bulk actions. The audit is
the list; it and the [security review](docs/rewrite/security-review.md) also
name two safety gaps — the socket bind is TOCTOU-racy, and the IPC accept loop
has no connection cap and no read or write timeouts.

Also absent: image, file and rich-text capture (text only), frontmost-app
attribution — which manifest 07 makes an independent *sensitivity* signal, not
just metadata — private mode, the app-exclusion list, rate limiting, and
telemetry. Two capabilities exist in `copypaste-core` with no caller:
`retention::evict_older_than` (age-based retention) and `page::list_from`
(keyset pagination — the wire and the app still page by offset).

## Build and run

MSRV 1.96 (`rust-version` in `Cargo.toml`; `rust-toolchain.toml` tracks
`stable`). SQLCipher is compiled from C source by `rusqlite`'s
`bundled-sqlcipher` feature, so the first build is slow and needs a C toolchain.
The Tauri crate is a workspace member, so `--workspace` also needs
`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev
libayatana-appindicator3-dev libxdo-dev` on Linux.

```sh
cargo build --release -p copypaste-daemon -p copypaste-cli
cargo test --workspace

./scripts/demo.sh          # capture → encrypt → store → search → paste back
./scripts/demo-p2p.sh      # two daemons, pairing, two-way convergence
./scripts/demo-cloud.sh    # two daemons against the local Supabase stub
```

The daemon never forks; backgrounding is the service manager's job.

```sh
target/release/copypaste-daemon --foreground &
target/release/copypaste list
```

On Linux the clipboard source is a fake, drivable from `COPYPASTE_FAKE_CLIPBOARD`
or over IPC, so the pipeline is demonstrable off a Mac. `copypaste status` always
names the live backend, so a demo cannot be mistaken for the real thing.

```sh
(cd crates/copypaste-ui && npm ci && npm run build && npm test)
(cd design && npm ci && npm run rebuild)   # tokens, then the contrast gate
```

`e2e/README.md` covers the real-WebView suite and what the host needs for it.

## The specification

`docs/rewrite/port-manifest/` is ~9,000 lines harvested from v0.4.1 and its
tests: ~500 acceptance tests and 200+ recovered bug ids. A subsystem is not done
until its manifest's tests pass.

Read [`port-manifest/README.md`](docs/rewrite/port-manifest/README.md) first. It
records, per manifest, which sections still bind and which became reference
material: dropping backward compatibility retired the *formats*, and rejecting
v1's design retired the *visuals*. Behaviour — platform quirks, security
properties, the accessibility contract, the detection ruleset — binds
throughout.

## Decisions

[`docs/README.md`](docs/README.md) indexes every ADR, audit and study, with the
question each one settles. Start there rather than here.

## Licence

MIT or Apache-2.0, at your option.
