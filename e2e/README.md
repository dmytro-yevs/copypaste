# e2e — the app in a real WebView

`npm test` builds nothing. It expects `copypaste-ui`, `copypaste-daemon` and
`copypaste` in `target/debug` (or `target/release`), and drives the app through
`tauri-driver` → `WebKitWebDriver` → the real wry WebView, talking to a real
daemon over a real Unix socket.

```
cargo build -p copypaste-ui -p copypaste-daemon -p copypaste-cli
cd crates/copypaste-ui && npm ci
cd e2e && npm ci && npm test
```

## What each layer proves, and what it does not

| Layer | Exercised | **Not** exercised |
|---|---|---|
| `crates/copypaste-ui` `npm test` (jsdom) | component logic, hooks, reducers | layout, scrolling, virtualisation, the Tauri bridge — jsdom has no box model and every rect is 0×0 |
| this suite | WebKit layout and paint, the virtualiser, keyboard and focus, `invoke` across the Tauri bridge, the daemon's IPC socket, the real SQLite store | macOS and Android. The engine here is WebKitGTK; macOS ships WKWebView and Android ships the system WebView. Tray, popover, global hotkey and launch-at-login are desktop-shell APIs this harness never touches |

A green run means the frontend, the bridge and the daemon agree on a Linux
host. It is not evidence about either shipping platform, and CLAUDE.md rule 7
means both still need their own verification.

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
| `devices` | real pairing against a **second real daemon**: mint, reveal, a wrong code, the right one, unpair — plus the QR (INV-13) and the no-camera fallback |
| `push` | a `copypaste://changed` event really crosses the bridge, the list updates inside the poll interval, and a dead daemon degrades to polling |
| `service-lifecycle` | the offline screen offers to *start* the service, and pressing the button really does |
| `settings` | every tab lays out, a preference reaches layout and survives a reload, and Settings still works with the service down |
| `export-import` | an export withholds and counts flagged items; an edited backup cannot import a credential marked clean |
| `daemon-config` | `GetConfig`/`SetConfig` over the socket — **no WebView**, because no Tauri command routes them (see below) |

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

## Surfaces this cannot reach yet

- **Configuration.** The daemon has `GetConfig`/`SetConfig` with per-field
  liveness and the CLI drives both, but no `#[tauri::command]` routes either, so
  Settings cannot read or change a single daemon setting. `daemon-config`
  exercises the contract through the CLI; when a command lands, those
  assertions belong in `settings`.
- **Export and import.** Same shape: `Method::Export` / `Method::Import` exist
  and the CLI uses them; the app has no backup or restore anywhere.
- Tray, popover, global hotkey, launch-at-login — desktop-shell APIs no
  WebDriver session touches.

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
