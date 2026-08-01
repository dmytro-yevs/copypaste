# CopyPaste

Encrypted clipboard manager for macOS and Android. One app — Tauri v2 + React —
on both platforms, over a shared Rust core. On desktop the app talks to a local
daemon; on Android it links the core in-process
([ADR-0003](docs/adr/0003-one-command-surface-two-backends.md)).

**v2.0.0-alpha.1, on the `v2-main` rewrite branch, unaudited.** `main` was reset
to an empty history on 2026-07-29; v0.4.1 remains intact at
`archive/v0.4.1-pre-rewrite` (2,153 commits). v2 reads nothing that version
wrote and uses a distinct database filename, `copypaste-v2.db`, so an old file
is never opened. [CLAUDE.md](CLAUDE.md) rule 3 has the reasoning and the one
obligation it creates.

The rewrite exists because v0.4.1 had grown to ~150k lines of Rust with six
retry implementations, three rate limiters and three models of one wire
contract. v2 inverts the norm that produced them: **a dependency is the default,
and hand-rolling needs a written reason.**

## Status

Three columns. The middle one is the point: *unverified* means written, compiled
and reviewed, and never observed doing its job on a platform we ship to.

[`docs/rewrite/testing-policy.md`](docs/rewrite/testing-policy.md) is the
authority for which layer establishes what. Every requirement has one
authoritative layer, and one that no run reaches is marked NOT VERIFIED IN CI
there rather than being credited to a layer that cannot see it.

### Works — covered by tests, and by the demo scripts where a script reaches

| Area | Notes |
|---|---|
| Crypto | XChaCha20-Poly1305 + HKDF-SHA256, item id bound as AAD, fail-closed, zeroized |
| Storage | One SQLCipher schema, r2d2 pool, FTS5 search, tombstones, pins, cap eviction |
| Secret detection | Ruleset sourced from gitleaks, NFKC normalisation, Luhn validation, confidence bands; a flagged item never reaches the index and never leaves the device. A purge pass at daemon start re-decides the index question for rows captured before a rule existed |
| Capture | Clipboard behind a trait, so `changeCount` detection, burst handling, self-write suppression and the `org.nspasteboard.*` opt-outs are all tested against the fake source on any host |
| IPC | `0600` Unix socket, newline-JSON, `LinesCodec` framing; `copypaste-ipc` is the only model of the contract, shared by daemon, CLI and the Tauri bridge |
| CLI | `crates/copypaste-cli/src/cli.rs` is the verb list — `copypaste --help` prints it. `--json` on any of them, for scripting |
| Peer sync | Noise `NNpsk0` over TCP, pairing codes, LWW merge, delete-wins |
| Cloud sync | Supabase auth, PostgREST, Realtime; rows sealed client-side under an Argon2id key and signed under a second key from the same passphrase, so the ordering metadata the backend pages on cannot be forged. Wired to the daemon and the CLI — but see below |
| App | History, search, devices/pairing, settings and the Android capture screens, driven in a browser engine. Every user-facing string is in one catalogue (`crates/copypaste-ui/src/i18n/`). The menu-bar item, popover, global hotkey and launch-at-login are written against Tauri plugins and belong to the row below |
| Design tokens | One DTCG source compiled by Style Dictionary; shadcn/ui on Tailwind v4, zinc base in OKLCH, with contrast measured and gated (`design/README.md`) |

### Unverified

| Thing | What is and is not established |
|---|---|
| The macOS shell — tray, popover, global hotkey, launch at login, notification and sound on copy, WKWebView | `macos-check` on `macos-14` runs the real `NSPasteboard` and the real Keychain on every push and pull request, and an empty run fails the job; those two are verified. Nothing anywhere registers a shortcut, posts a notification or renders a frame on WKWebView. |
| Android beyond launch and storage | The nightly emulator run installs both APKs, and it establishes launch, a painted WebView, the Keystore secret surviving a restart, an unreadable SQLCipher file, R8 and signing. Rung 2 (the Shizuku shell-uid read), the background capture service, the Quick Settings tile and `FLAG_SECURE` are asserted only negatively or not at all; [`docs/rewrite/android-spike.md`](docs/rewrite/android-spike.md) lists what a first device run would falsify. |
| Cloud sync against Supabase | `scripts/demo-cloud.sh` drives two daemons through sign-in, convergence and sensitive-item refusal against a **local stub** (`scripts/cloud-stub.py`), and no workflow runs it. Nothing has ever spoken to a real project, and no deployment has had `supabase/`'s schema and RLS policies applied. |
| The app on a shipping engine | The `e2e/` suite drives the built app through `tauri-driver` → `WebKitWebDriver` under Xvfb, and WebKitGTK 2.52 does execute JavaScript and compute layout there. That is the browser layer: it establishes the shared React app's behaviour and nothing about WKWebView or the Android WebView. |
| Packaging and release | `release.yml` builds, signs and smoke-installs the DMG on `macos-14`, but only on a tag — so `codesign`, `hdiutil` and the Tauri bundler never run on a pull request, and the smoke script's app-launch and Keychain-after-resign legs report rather than fail. `brew install --cask` as a user runs it is unexercised; `check.sh` round-trips the generators. |
| mDNS discovery | This container has no multicast. Discovery is a convenience; an explicit `--addr` always works and is what the demo and the tests use. |

### Product limits

CopyPaste is text-only: it does not capture images, files, rich text, or the
frontmost application. Manifest 07 treats source application as an independent
sensitivity signal rather than item metadata.

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

`e2e/README.md` covers the browser-layer suite and what the host needs for it.

## The specification

`docs/rewrite/port-manifest/` is harvested from v0.4.1 and its tests: ~500
acceptance tests and 200+ recovered bug ids. A subsystem is not done until its
manifest's tests pass.

Read [`port-manifest/README.md`](docs/rewrite/port-manifest/README.md) first. It
records, per manifest, which sections still bind and which became reference
material: dropping backward compatibility retired the *formats*, and rejecting
v1's design retired the *visuals*. Behaviour — platform quirks, security
properties, the accessibility contract, the detection ruleset — binds
throughout.

## Decisions

[`docs/README.md`](docs/README.md) indexes decisions, requirements and operating
guides.

## Licence

MIT or Apache-2.0, at your option.
