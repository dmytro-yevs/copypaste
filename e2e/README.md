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
