# CopyPaste

Encrypted clipboard manager for macOS, Android and Windows
([ADR-0013](docs/adr/0013-windows-as-a-third-platform.md)). One app — Tauri v2 +
React — over a shared Rust core. On desktop the app talks to a local daemon; on
Android it links the core in-process
([ADR-0003](docs/adr/0003-one-command-surface-two-backends.md)).

**Early v2 prerelease, on `main`, unaudited.** The
[releases page](https://github.com/dmytro-yevs/copypaste/releases) lists v2
prereleases newest first; GitHub's `releases/latest` redirect selects the
legacy stable line. See the
[Android installation notes](packaging/release-notes.md#android) for the current
release asset. CopyPaste is under active development.
Dependencies are the default; hand-rolled infrastructure needs a written reason.

## Status

Three columns. The middle one is the point: *unverified* means written, compiled
and reviewed, and never observed doing its job on a platform we ship to.

[`docs/rewrite/testing-policy.md`](docs/rewrite/testing-policy.md) is the
authority for which layer establishes what. Every requirement has one
authoritative layer, and one that no run reaches is marked NOT VERIFIED IN CI
there rather than being credited to a layer that cannot see it.

Windows clipboard capture, DPAPI secret storage and owner-only named-pipe IPC
are implemented. The shell, packaging and release verification remain tracked
separately ([ADR-0013](docs/adr/0013-windows-as-a-third-platform.md)).

### Works — covered by tests, and by the demo scripts where a script reaches

| Area | Notes |
|---|---|
| Crypto | XChaCha20-Poly1305 + HKDF-SHA256, item id bound as AAD, fail-closed, zeroized |
| Storage | One SQLCipher schema, r2d2 pool, FTS5 search, tombstones, pins, cap eviction |
| Secret detection | Ruleset sourced from gitleaks, NFKC normalisation, Luhn validation, confidence bands; a flagged item never reaches the index and never leaves the device. A purge pass at daemon start re-decides the index question for rows captured before a rule existed |
| Capture | Native macOS and Windows capture behind one trait, including sequence-based change detection, burst handling, self-write suppression and platform opt-outs |
| IPC | Owner-only Unix socket or Windows named pipe, newline-JSON, `LinesCodec` framing; `copypaste-ipc` is the shared contract used by daemon, CLI and Tauri |
| CLI | `crates/copypaste-cli/src/cli.rs` is the verb list — `copypaste --help` prints it. `--json` on any of them, for scripting |
| Peer sync | Noise `NNpsk0` over TCP, pairing codes, LWW merge, delete-wins |
| Cloud sync | Supabase auth, PostgREST, Realtime; rows sealed client-side under an Argon2id key and signed under a second key from the same passphrase, so the ordering metadata the backend pages on cannot be forged. Wired to the daemon and the CLI — but see below |
| App | History, search, devices/pairing, settings and the Android capture screens, driven in a browser engine. Every user-facing string is in one catalogue (`crates/copypaste-ui/src/i18n/`). The menu-bar item, popover, global hotkey and launch-at-login are written against Tauri plugins and belong to the row below |
| Design tokens | One DTCG source compiled by Style Dictionary; shadcn/ui on Tailwind v4, zinc base in OKLCH, with contrast measured and gated (`design/README.md`) |

### Unverified

| Thing | What is and is not established |
|---|---|
| The macOS shell — tray, popover, global hotkey, launch at login, notification and sound on copy, WKWebView | `macos-check` on `macos-14` runs the real `NSPasteboard` and the real Keychain on every push and pull request, and an empty run fails the job; those two are verified. Nothing anywhere registers a shortcut, posts a notification or renders a frame on WKWebView. |
| Android beyond launch and storage | The emulator matrix installs debug and release x86_64 test APKs. It establishes launch, a painted WebView, Keystore persistence, SQLCipher storage, R8 and signing. Its API 36 leg also proves Quick Settings tile capture, fail-closed background-service state and `FLAG_SECURE`. The app-owned ClipCascade path still needs device evidence for `READ_LOGS`, overlay focus, Android 12+ privacy UI and OEM battery managers; [`docs/rewrite/android-spike.md`](docs/rewrite/android-spike.md) lists the remaining device evidence. |
| Cloud sync against Supabase | Two layers, established separately. The stronger one is the release gate: `release.yml`'s `supabase-gate` job runs `supabase/tests/real-supabase.sh`, which brings up a **disposable local Supabase stack** through the Supabase CLI, applies `supabase/`'s migrations with `supabase db reset`, asserts schema, RLS behaviour and retention/paging from SQL, then drives auth, PostgREST and Realtime through the `real_supabase_contract` integration test — and blocks publication when it fails. It runs only on the release workflow, so no pull request exercises it. The weaker one is the demo: `scripts/demo-cloud.sh` drives two daemons through sign-in, convergence and sensitive-item refusal against a **local stub** (`scripts/cloud-stub.py`), and no workflow runs it. Neither layer leaves the runner: no hosted or production Supabase project is part of any check. |
| The app on a shipping engine | The `e2e/` suite drives the built app through `tauri-driver` → `WebKitWebDriver` under Xvfb, and WebKitGTK 2.52 does execute JavaScript and compute layout there. That is the browser layer: it establishes the shared React app's behaviour and nothing about WKWebView or the Android WebView. |
| Packaging and release | `release.yml` builds, signs and smoke-installs the DMG on `macos-14`, but only on a tag — so `codesign`, `hdiutil` and the Tauri bundler never run on a pull request, and the smoke script's app-launch and Keychain-after-resign legs report rather than fail. `brew install --cask` as a user runs it is unexercised; `check.sh` round-trips the generators. |
| mDNS discovery | This container has no multicast. Discovery is a convenience; an explicit `--addr` always works and is what the demo and the tests use. |

### Product limits

Native clipboard capture takes one representation, plain text: a change that
offers only an image, a file reference or rich text is acknowledged and skipped
(`crates/copypaste-daemon/src/clipboard/format.rs`). Non-text content is first
class everywhere else: `crates/copypaste-ipc/src/content_type.rs` is the
vocabulary, naming RTF, HTML, PNG, TIFF and file references, and an item of
those types arriving from a paired peer or from cloud sync is stored,
size-gated under its own limit class, listed, and pasted back to the system
clipboard in its original representation. An image row also serves a preview
to the app.

Frontmost application identity is retained as provenance and as an independent
sensitivity signal.

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

### Run the app while developing

`npm run dev` is a **web-only UI preview**. It deliberately has no Tauri IPC,
macOS clipboard service, Keychain, tray, or Android backend, so its offline and
empty-history states are expected. Use it only for layout work that does not
need native behaviour.

For a browser preview against a local macOS daemon, use the development-only
adapter instead:

```sh
cd crates/copypaste-ui
npm run dev:web:daemon
```

It starts a daemon and a loopback-only bridge on an ephemeral port. The bridge
creates a one-run bearer token, allows requests only from Vite's
`http://localhost:1420` origin, and exposes only the safe history/status/
settings/peer actions needed by the preview. It is not compiled into Tauri,
release, or Android builds. The adapter deliberately refuses sensitive-item
reveal, raw clipboard reads, file panels, and every production-only native
command.

On macOS, this starts a debug daemon, records its structured events locally,
and opens the Tauri app against that daemon:

```sh
cd crates/copypaste-ui
npm run dev:native
```

The command builds the daemon first, writes one JSON-lines session log under
`target/copypaste-dev/`, then stops only the daemon it started when the native
app exits. Set `COPYPASTE_DEV_RUST_LOG=copypaste_daemon=info` for quieter logs.
This is developer-only output; the in-app diagnostic export is a separate,
redacted support artifact and must never contain clipboard contents or local
paths.

Android has no daemon: it runs the shared core in the app process. With an
Android SDK/NDK and an emulator or device attached through `adb`, run:

```sh
cd crates/copypaste-ui
npm run dev:android
```

For Android platform logs, use the package PID so unrelated device logs do not
get mixed in:

```sh
adb logcat --pid="$(adb shell pidof -s com.copypaste.app)"
```

## The specification

`docs/rewrite/port-manifest/` contains ~500 acceptance tests and 200+ recovered
bug ids. A subsystem is not done until its manifest's tests pass.

Read [`port-manifest/README.md`](docs/rewrite/port-manifest/README.md) first. It
records, per manifest, which sections bind. Platform quirks, security
properties, the accessibility contract and the detection ruleset bind
throughout.

## Decisions

[`docs/README.md`](docs/README.md) indexes decisions, requirements and operating
guides.

## Licence

MIT or Apache-2.0, at your option.
