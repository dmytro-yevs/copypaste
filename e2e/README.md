# e2e — the shared frontend in a Linux browser engine

This is the **browser layer** of [`docs/rewrite/testing-policy.md`](../docs/rewrite/testing-policy.md),
run by `.github/workflows/browser-webkitgtk.yml`. It owns shared React
behaviour — rendering, layout, navigation, forms and dialogs, empty and error
states, keyboard navigation, focus, the accessibility tree, overflow — and it
owns nothing native. The engine is WebKitGTK on Linux.

`npm test` builds nothing. It expects `copypaste-ui`, `copypaste-daemon` and
`copypaste` in `target/debug` (or `target/release`), and drives the app through
`tauri-driver` → `WebKitWebDriver` → the wry WebView, talking to a real daemon
over a real Unix socket.

```
cargo build -p copypaste-ui -p copypaste-daemon -p copypaste-cli
cd crates/copypaste-ui && npm ci
cd e2e && npm ci && npm test
```

## What each layer proves, and what it does not

| Layer | Exercised | **Not** exercised |
|---|---|---|
| `crates/copypaste-ui` `npm test` (jsdom) | component logic, hooks, reducers | layout, scrolling, virtualisation, anything crossing the Tauri bridge — jsdom has no box model and every rect is 0×0 |
| this suite | WebKit layout and paint, the virtualiser, keyboard and focus, the accessibility tree, the daemon's IPC socket and the real SQLite store behind it | macOS and Android. The engine here is WebKitGTK; macOS ships WKWebView and Android ships the system WebView. A Tauri command runs here through `wry` on Linux, which the policy does not accept as verification of that command as shipped. Tray, popover, global hotkey and launch-at-login are desktop-shell APIs this harness never touches |

A green run means the shared frontend and the daemon agree on a Linux host. It
is not evidence about either shipping platform; the layer that owns each native
requirement, and the ones marked NOT VERIFIED IN CI, are in the policy.

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
| `push` | a `copypaste://changed` event crosses the Linux bridge, the list updates inside the poll interval, and a dead daemon degrades to polling |
| `service-lifecycle` | the offline screen offers to *start* the service, and pressing the button starts a Linux `target/debug` daemon — launchd and Homebrew belong to the macOS layer |
| `settings` | every tab lays out, a preference reaches layout and survives a reload, and Settings still works with the service down |
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
- **Configuration from the screen.** `commands/config.rs` routes
  `GetConfig`/`SetConfig` and `ServiceTab` calls them, but `daemon-config`
  asserts the contract through the CLI and `ServiceTab.test.tsx` mocks both
  calls. The screen's path to the daemon is NOT VERIFIED IN CI; when it is
  driven, those assertions belong in `settings`.
- **Export, import, backup, restore from the screen.** Same shape:
  `commands/transfer.rs` ships all four and `StorageTab` surfaces them, while
  `export-import` goes through `copypaste export`. NOT VERIFIED IN CI.
- Tray, popover, global hotkey, launch-at-login — desktop-shell APIs no
  WebDriver session touches.
- Everything the policy assigns to the macOS or Android layer. Nothing here
  substitutes for either.

## Requirements the host must satisfy

- `/usr/bin/WebKitWebDriver`, from the `webkit2gtk-driver` package. Separate
  from the webkit runtime and absent by default.
- `tauri-driver` — `cargo install tauri-driver --locked`.
- An X display. `npm test` wraps vitest in `xvfb-run`; `npm run test:headed`
  does not, for a real display.

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
