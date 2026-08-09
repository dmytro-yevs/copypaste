# e2e — the desktop app through native WebDriver

The same WebdriverIO harness runs in two layers from
[`docs/rewrite/testing-policy.md`](../docs/rewrite/testing-policy.md):
`.github/workflows/browser-webkitgtk.yml` drives shared React behaviour in
WebKitGTK on Linux, while `.github/workflows/windows-native-e2e.yml` drives the
shipped Windows WebView2 and Tauri bridge.

`npm test` builds nothing. It expects `copypaste-ui`, `copypaste-daemon` and
`copypaste` in `target/debug` (or `target/release`), and drives the app through
`tauri-driver` → `WebKitWebDriver` → the wry WebView, talking to a real daemon
over a real Unix socket.

```
cargo build -p copypaste-ui -p copypaste-daemon -p copypaste-cli
cd crates/copypaste-ui && npm ci
cd e2e && npm ci && npm test
```

On Windows use `npm run test:windows`. The hosted workflow supplies
`EDGEWEBDRIVER` and opts into the shell-setting tests, which temporarily change
the disposable runner account's shortcut and launch-at-login registration.

## What each layer proves, and what it does not

| Layer | Exercised | **Not** exercised |
|---|---|---|
| `crates/copypaste-ui` `npm test` (jsdom) | component logic, hooks, reducers | layout, scrolling, virtualisation, anything crossing the Tauri bridge — jsdom has no box model and every rect is 0×0 |
| this suite on Linux | WebKit layout and paint, the virtualiser, keyboard and focus, the accessibility tree, the Unix socket and real SQLite store | every shipping platform and every native desktop surface |
| this suite on Windows | WebView2 layout, the shipped Tauri command bridge, named-pipe IPC, native clipboard capture, shortcut and autostart registration state | tray interaction, OS hotkey delivery, execution after sign-in, notifications, native file pickers |

A green Linux run proves the shared frontend. A green Windows run additionally
proves the shipped Windows WebView and bridge, but only for the native surfaces
the suite actually observes. The policy keeps the remaining gaps explicit.

## What is driven

Every file starts its own app: WebKitWebDriver answers a second concurrent
session with "Maximum number of active sessions", so the files run one at a
time and each gets a fresh daemon and a fresh database.

| File | What only a running program can show |
|---|---|
| `smoke` | the app mounts, paints and reaches the daemon at all |
| `history-render` · `scroll-anchor` · `history-controls` | virtualisation, the row-height rule (INV-5), scroll anchoring (INV-1/6), keyboard navigation, toolbar layout |
| `sensitive` · `error-strings` | a flagged item's plaintext is absent from `outerHTML`, and no user-facing string carries a filesystem path (INV-10/12) |
| `bulk-actions` | per-row actions are **absent** in selection mode rather than hidden, and a bulk delete reaches the database |
| `devices` | the ADR-0015 boundary in a real engine: pairing-unavailable copy and Pair/Add/code/QR controls absent; a CLI-established peer is listed, syncs, exposes the revoke confirmation and unpairs |
| `push` | a `copypaste://changed` event crosses the host's Tauri bridge, the list updates inside the poll interval, and a dead daemon degrades to polling |
| `service-lifecycle` | the offline screen offers to *start* the service, and pressing the button starts the sibling `target/debug` daemon |
| `settings` | every tab lays out, a daemon preference reaches the service, an app preference reaches layout and survives a reload, and Settings still works with the service down |
| `windows-surfaces` | private mode blocks the native clipboard, cloud stays closed when unconfigured, transfer warnings, shortcut/autostart registration, diagnostics and runtime logs |
| `export-import` | an export withholds and counts flagged items; an edited backup cannot import a credential marked clean — driven through the CLI, not through Settings (see below) |
| `daemon-config` | `GetConfig`/`SetConfig` over the socket — **no WebView**, driven through the CLI (see below) |

Two constraints are asserted from `src/harness/leaks.ts` rather than restated
per file: a secret must be absent from `outerHTML` (a blur, a `display: none`
or an `aria-label` all leave it there), and no accessible string — text,
`title`, `aria-label`, `placeholder`, `value` — may contain a filesystem path,
because the daemon socket path discloses the local username.

Assertions are against **rendered text**, never a catalogue key, so they survive
strings moving into `src/i18n` and fail if a key ever reaches the screen.

## Known red

`daemon-config` › "patches over different fields all survive" fails, and the
defect is in the daemon rather than in the test. `Settings::apply`
(`crates/copypaste-daemon/src/settings.rs`) reads the live configuration under a
read lock, drops it, validates the patch into a new value, persists that, and
only then takes the write lock — so two connections that overlap read the same
"before" and the second write erases the field the first one set. The test
writes four fields from four clients per round, six rounds, and reports every
field that did not survive.

## Surfaces this suite does not reach

- **An in-product pairing ceremony.** ADR-0015 requires Pair and Add-device
  controls to remain absent until the protocol supplies a bound SAS ceremony.
  `devices` establishes a known-peer fixture through the CLI; that setup is not
  browser coverage for code mint/reveal, QR (INV-13), camera fallback or SAS.
- **Export, import, backup, restore from the screen.** Same shape:
  `commands/transfer.rs` ships all four and `StorageTab` surfaces them, while
  `export-import` goes through `copypaste export`. The Windows file asserts the
  safety confirmations, but WebDriver cannot control the native file pickers.
- Tray and popover interaction, OS hotkey delivery and post-sign-in launch —
  native lifecycle events no WebDriver session observes.
- Everything the policy assigns to the macOS or Android layer. Nothing here
  substitutes for either. The Android WebView has its own driven harness in
  [`e2e-android/`](../e2e-android/README.md); WKWebView has none.

## Requirements the host must satisfy

- `/usr/bin/WebKitWebDriver`, from the `webkit2gtk-driver` package. Separate
  from the webkit runtime and absent by default.
- On Windows, a matching `msedgedriver.exe`. GitHub's hosted Windows image
  exposes its directory through `EDGEWEBDRIVER`.
- `tauri-driver` — `cargo install tauri-driver --locked`.
- An X display on Linux. `npm test` wraps vitest in `xvfb-run`; Windows runs
  directly in the hosted interactive session.

No GPU is needed and no software-rendering flags are set. `LIBGL_ALWAYS_SOFTWARE`,
`WEBKIT_DISABLE_COMPOSITING_MODE` and `WEBKIT_DISABLE_DMABUF_RENDERER` were each
tried and none is load-bearing — WebKitGTK 2.52 runs JavaScript and computes
layout under plain Xvfb. The `libEGL … DRI3` lines on stderr are cosmetic.

## Constraints worth knowing before editing the harness

- **One session at a time.** WebKitWebDriver answers a second concurrent
  `POST /session` with "Maximum number of active sessions", so
  `fileParallelism` is off.
- **Run directories live under `/tmp/cp-e2e`.** The daemon's socket path comes
  from `XDG_DATA_HOME`, and a socket path over ~108 bytes fails to bind with
  "path must be shorter than SUN_LEN". Deeper scratch directories break it.
- **A debug build loads `devUrl`.** Global setup runs the Vite dev server on
  1420 and waits for `/src/main.tsx` to be served, not merely for the port to
  answer: the root URL responds before dependency pre-bundling finishes, and a
  page that loads inside that window gets a module graph that fails to import.
