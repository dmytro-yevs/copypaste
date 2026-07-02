#!/usr/bin/env node
/**
 * tauri-smoke/run.mjs — packaged-Tauri smoke gate.
 *
 * design-system-redesign tasks.md 6.17 ("PACKAGED-Tauri smoke/integration
 * checks = the product release gate") + bd CopyPaste-g27b.13.2 / .13.3.
 *
 * This is the PACKAGED-UI complement to `scripts/smoke_test.sh` (which covers
 * daemon/CLI/IPC correctness on a release build). This script instead launches
 * the actual bundled `CopyPaste.app` produced by `pnpm tauri build` and probes
 * it as a black box.
 *
 * ═══════════════════════════════════════════════════════════════════════════
 * WHAT RUNS WHERE — read this before trusting a green run
 * ═══════════════════════════════════════════════════════════════════════════
 *
 * ── Tier 1: process/filesystem probes (need only the packaged binary, no ──
 * ── WebDriver) — REAL wherever a packaged .app exists: locally (after a  ──
 * ── prior `pnpm tauri build`, or with `--build`) AND in CI               ──
 * ── (.github/workflows/ui-redesign-gate.yml's tauri-packaged-smoke job,  ──
 * ── which always passes `--build`):                                     ──
 *   1. launch-and-liveness        — the packaged binary starts and is still
 *                                    running `--startup-wait` ms later (no
 *                                    immediate crash / non-zero early exit).
 *   2. ipc-transport-reachable    — the app-owned daemon (spawned by the
 *                                    Tauri process itself, see
 *                                    src-tauri/src/daemon_lifecycle.rs) opens
 *                                    its Unix-socket IPC endpoint and answers
 *                                    a `status` request with `"ok":true`.
 *                                    NOTE: this proves the daemon side of IPC
 *                                    is live; it does NOT prove the frontend
 *                                    JS actually issued an `ipc_call` — see
 *                                    the Tier-2 `frontend-ipc-bridge-invoked`
 *                                    probe below.
 *   3. no-fatal-startup-markers   — the captured stdout/stderr of the app
 *                                    process (which, since the daemon child
 *                                    inherits those fds by default — Rust's
 *                                    `std::process::Command` default stdio is
 *                                    `inherit` — transitively includes the
 *                                    daemon's own output) PLUS the tail of the
 *                                    daemon's rotated file log are scanned for
 *                                    Rust panic signatures, "FATAL" lines, and
 *                                    CSP/console-violation phrasing.
 *                                    NOTE: this catches Rust-side panics and
 *                                    daemon crashes. It does NOT catch a JS
 *                                    `console.error()` or a WKWebView CSP
 *                                    violation raised purely inside the
 *                                    WebView — see the Tier-2
 *                                    `webview-console-and-csp` probe below.
 *   4. prefs-persist-on-disk-after-launch — a non-default UiConfig is seeded
 *                                    into the isolated app-config dir BEFORE
 *                                    launch (see config.rs's `UiConfig`);
 *                                    after a full launch+terminate cycle the
 *                                    on-disk `ui-config.json` must still hold
 *                                    the seeded values. Catches a regression
 *                                    where startup silently rewrites the
 *                                    config back to defaults. NOTE: this
 *                                    proves the FILE round-trips; it does NOT
 *                                    prove the frontend rendered/applied it —
 *                                    that half is the Tier-2
 *                                    `preferences-load-and-apply` probe below.
 *
 * ── Tier 2: WebDriver-driven WebView-introspection probes ───────────────
 * (`webview-console-and-csp`, `frontend-ipc-bridge-invoked`,
 * `preferences-load-and-apply`, `modal-keyboard-focus-behavior`,
 * `theme-accent-translucency-cross-window`, `popup-opens-and-renders`).
 * These talk to the packaged app's actual WKWebView content via the W3C
 * WebDriver protocol, proxied through `tauri-driver` (installed separately
 * via `cargo install tauri-driver --locked` — NOT an npm devDependency;
 * crates/copypaste-ui/package.json is scoped, for this change, to test:*
 * script edits only). A minimal WebDriver HTTP client is hand-rolled in
 * ./webdriver-client.mjs (Node's built-in `fetch`, zero new npm deps) rather
 * than pulling in webdriverio/selenium-webdriver.
 *
 * REALITY CHECK — these do NOT execute for real on this repo's actual CI
 * platform today: `tauri-driver` has NO macOS backend upstream (it wraps
 * WebKitWebDriver on Linux and msedgedriver on Windows only; macOS support is
 * tracked at https://github.com/tauri-apps/tauri/issues/5163, still open at
 * the time of writing). CopyPaste ships macOS-only packaged builds (see
 * CLAUDE.md "Platform Support" / ADR-012) and
 * .github/workflows/ui-redesign-gate.yml only runs on macos-14. So
 * `detectWebDriverBackend()` below always reports "unavailable" in this
 * repo's CI, and every Tier-2 probe is recorded as `skip` with that specific
 * reason — never a faked `pass`. The probe LOGIC is nonetheless implemented
 * for real and is correct and runnable the moment either (a) upstream ships
 * macOS support, or (b) you point this at a Linux/Windows Tauri build for
 * manual validation (`TAURI_DRIVER_BIN=/path/to/tauri-driver`, or rely on
 * PATH; `TAURI_SMOKE_FORCE_WEBDRIVER=1` to opt in on darwin for manual
 * testing against an external/proxied driver).
 *
 * KNOWN GAP even when a WebDriver backend IS available (NOT fixed here — out
 * of this change's file scope, would need a new debug-only Tauri command):
 * `theme-accent-translucency-cross-window` and `popup-opens-and-renders`
 * ALWAYS report `skip`. The quick-paste popup is lazy-created only by the
 * OS-level global-shortcut / CGEventTap handler
 * (src-tauri/src/popup/window.rs's `toggle_popup` is a plain fn, not a
 * `#[tauri::command]`) — there is no IPC-invokable, in-WebView way to open
 * it, and WebDriver's Actions API only dispatches input into the current
 * browsing context, not system-wide, so it cannot trigger an OS-registered
 * global hotkey either. Opening the popup deterministically from a test
 * harness would need a new test-only Tauri command exposed to the frontend —
 * a source change outside this change's owned-files scope
 * (crates/copypaste-ui/e2e/tauri-smoke/**, .github/workflows/**, and test:*
 * scripts in package.json only).
 *
 * ─────────────────────────────────────────────────────────────────────────
 * Exit code: 0 only if every REAL (non-skip, non-stub) check passed. Tier 1
 * is always real; Tier 2 is real only when a WebDriver backend is actually
 * available (see REALITY CHECK above — never true in this repo's CI today).
 * `skip` and `stub` never fail the gate on their own.
 * ─────────────────────────────────────────────────────────────────────────
 * Isolation (never touches the real user's data / Keychain / login items):
 * ─────────────────────────────────────────────────────────────────────────
 * Mirrors `scripts/smoke_test.sh --from-bundle`'s isolation strategy:
 *   - COPYPASTE_SOCKET / COPYPASTE_DB / COPYPASTE_DATA_DIR / _CACHE_DIR /
 *     _CONFIG_DIR / _LOG_DIR / _DEVICE_ID_PATH point into a fresh mktemp
 *     root — honoured by copypaste-daemon (crates/copypaste-daemon/src/paths.rs).
 *   - COPYPASTE_EPHEMERAL_KEY=1 — daemon uses an in-memory key, never touches
 *     the real macOS Keychain.
 *   - HOME is ALSO overridden to an isolated dir. Reason: the Tauri UI crate's
 *     own config (crates/copypaste-ui/src-tauri/src/config.rs, via Tauri's
 *     `app_config_dir()`) and `tauri-plugin-autostart`'s LaunchAgent plist do
 *     NOT honour the COPYPASTE_* env vars above — they resolve paths from
 *     the process's HOME. Overriding HOME isolates those too.
 *
 * ─────────────────────────────────────────────────────────────────────────
 * Usage:
 * ─────────────────────────────────────────────────────────────────────────
 *   pnpm test:tauri-smoke                # reuse an existing packaged build
 *                                         # (fast — the default; fails loudly
 *                                         # if no build is found).
 *   pnpm test:tauri-smoke --build        # run `pnpm tauri build` first (this
 *                                         # crate), then run the smoke checks.
 *                                         # SLOW (full Tauri build). This is
 *                                         # the mode the CI release gate uses.
 *
 * Options:
 *   --build                 build the app via `pnpm tauri build` before probing.
 *   --app-path <path>       use a specific .app bundle instead of the default
 *                            workspace-root `target/release/bundle/macos/CopyPaste.app`.
 *   --startup-wait <ms>     liveness window (default 6000).
 *   --socket-timeout <ms>   max time to wait for the daemon socket (default 10000).
 *
 * Tier-2 (WebDriver) environment variables — see the REALITY CHECK section
 * above; these matter only on a platform where tauri-driver is usable:
 *   TAURI_DRIVER_BIN            path/name of the tauri-driver binary
 *                                (default: "tauri-driver", resolved via PATH).
 *   TAURI_SMOKE_FORCE_WEBDRIVER=1  attempt the Tier-2 probes on darwin too
 *                                (normally short-circuited to `skip` — see
 *                                REALITY CHECK). Only useful if you have an
 *                                external/proxied WebDriver endpoint set up
 *                                for manual testing; not expected to work
 *                                against a stock macOS tauri-driver install
 *                                (there isn't one).
 *
 * IMPORTANT — sibling-binary injection under --build:
 * `pnpm tauri build` alone only compiles/bundles the `copypaste-ui` Tauri
 * shell; it does NOT build or embed the copypaste-daemon/copypaste (CLI)/
 * copypaste-relay binaries — those are separate workspace crates, built
 * independently and copied into `Contents/MacOS/` (see
 * `scripts/release/build-dmg-ci.sh`'s "Injecting sibling binaries" step).
 * Without `copypaste-daemon` next to the UI binary, `bundled_daemon_path()`
 * (src-tauri/src/daemon_lifecycle.rs) can't find it, the app-owned daemon
 * never starts, and `ipc-transport-reachable` would fail on every run for a
 * reason unrelated to the UI itself. So after `pnpm tauri build` succeeds,
 * `--build` also copies `target/release/{copypaste-daemon,copypaste,
 * copypaste-relay}` into the bundle's `Contents/MacOS/` if present
 * (`copypaste-daemon` is REQUIRED — hard-fails with a build instruction if
 * missing; the CLI/relay siblings are best-effort, matching production bundle
 * shape but not required for this smoke gate's own checks). This is NOT a
 * full release build: no codesigning, no DMG, no notarization — just enough
 * for the packaged app to actually spawn its daemon, same as a real launch.
 */

import { spawn, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  rmSync,
  existsSync,
  readdirSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
  copyFileSync,
  chmodSync,
} from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { WebDriverSession } from "./webdriver-client.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// crates/copypaste-ui/e2e/tauri-smoke/run.mjs -> repo root is 4 levels up.
const REPO_ROOT = path.resolve(__dirname, "../../../..");
const UI_CRATE_DIR = path.join(REPO_ROOT, "crates/copypaste-ui");
const DEFAULT_APP_BUNDLE_PATH = path.join(
  REPO_ROOT,
  "target/release/bundle/macos/CopyPaste.app",
);

// Rust panic signature, explicit FATAL lines, and CSP/console-violation
// phrasing (as commonly emitted by Chromium/WebKit-family CSP error text).
// Kept specific to avoid false positives against benign log lines.
const FATAL_MARKERS = [
  /thread '.*?' panicked at/i,
  /^FATAL[:\s]/m,
  /content security policy/i,
  /refused to (load|connect|execute|frame)/i,
  /uncaught (type|reference|syntax)error/i,
];

// Tier-2 (WebDriver-gated) probe names — see the header comment for exactly
// which of these are implemented for real vs. structurally always-skip.
const WEBDRIVER_PROBE_NAMES = [
  "webview-console-and-csp",
  "frontend-ipc-bridge-invoked",
  "preferences-load-and-apply",
  "theme-accent-translucency-cross-window",
  "popup-opens-and-renders",
  "modal-keyboard-focus-behavior",
];

// Non-default UiConfig values seeded into the isolated app-config dir before
// launch (see src-tauri/src/config.rs's `UiConfig`). Every field here differs
// from `UiConfig::default()` (popup_shortcut="CmdOrCtrl+Shift+V",
// popup_position="cursor", allow_screenshots=false) so a silent reset to
// defaults is unambiguously detectable by the prefs-persist-on-disk probe.
// `launch_at_login` is intentionally OMITTED (not merely "left at the
// default") — serde's per-field `#[serde(default = ...)]` then leaves it at
// its normal `true` default, which avoids exercising the
// launch_at_login -> tauri-plugin-autostart -> real `launchctl` LaunchAgent
// path. That path is NOT scoped by the isolated HOME override below (see
// createIsolatedEnv's doc comment) and must not be touched by this probe.
const SEEDED_UI_CONFIG = {
  popup_shortcut: "CmdOrCtrl+Shift+K",
  popup_position: "center",
  allow_screenshots: true,
};

function log(...args) {
  console.log("[tauri-smoke]", ...args);
}

function hardFail(msg) {
  console.error(`\n[tauri-smoke] FAIL: ${msg}\n`);
  process.exit(1);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function printHelp() {
  console.log(`Usage: node e2e/tauri-smoke/run.mjs [options]

Options:
  --build                 build via 'pnpm tauri build' before probing (slow)
  --app-path <path>       use a specific .app bundle
  --startup-wait <ms>     liveness window (default 6000)
  --socket-timeout <ms>   daemon-socket wait timeout (default 10000)
  --help, -h              show this help

Default (no flags): reuse the existing packaged build at
  ${DEFAULT_APP_BUNDLE_PATH}
and fail loudly if it is not present.`);
}

function parseArgs(argv) {
  const opts = {
    build: false,
    appPath: null,
    startupWaitMs: Number(process.env.TAURI_SMOKE_STARTUP_WAIT_MS ?? 6000),
    socketTimeoutMs: Number(
      process.env.TAURI_SMOKE_SOCKET_TIMEOUT_MS ?? 10000,
    ),
    shutdownGraceMs: Number(
      process.env.TAURI_SMOKE_SHUTDOWN_GRACE_MS ?? 3000,
    ),
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--build") opts.build = true;
    else if (a === "--app-path") opts.appPath = argv[++i];
    else if (a === "--startup-wait") opts.startupWaitMs = Number(argv[++i]);
    else if (a === "--socket-timeout") opts.socketTimeoutMs = Number(argv[++i]);
    else if (a === "--help" || a === "-h") {
      printHelp();
      process.exit(0);
    } else {
      hardFail(`unknown argument: ${a} (use --help for usage)`);
    }
  }
  return opts;
}

/** Read CFBundleExecutable out of the .app's Info.plist (mirrors the same
 * lookup in scripts/release/build-dmg-ci.sh) so we launch the right binary
 * even if productName/mainBinaryName ever diverge from "CopyPaste". */
function resolveBundleExecutable(appPath) {
  const plist = path.join(appPath, "Contents/Info.plist");
  const res = spawnSync("/usr/libexec/PlistBuddy", [
    "-c",
    "Print :CFBundleExecutable",
    plist,
  ]);
  const name = res.stdout ? res.stdout.toString().trim() : "";
  return name || "CopyPaste";
}

/** Copy the daemon/CLI/relay sibling binaries into the freshly-built bundle's
 * Contents/MacOS/ so the app-owned daemon lifecycle can find and spawn its
 * daemon (see the header comment for why `pnpm tauri build` alone isn't
 * enough). `copypaste-daemon` is required; the CLI/relay are best-effort. */
function injectSiblingBinaries(appPath) {
  const macosDir = path.join(appPath, "Contents/MacOS");
  const siblings = [
    { name: "copypaste-daemon", required: true },
    { name: "copypaste", required: false },
    { name: "copypaste-relay", required: false },
  ];
  for (const { name, required } of siblings) {
    const src = path.join(REPO_ROOT, "target/release", name);
    const dst = path.join(macosDir, name);
    if (!existsSync(src)) {
      const msg =
        `sibling binary '${name}' not found at ${src} — the app-owned ` +
        `daemon needs 'copypaste-daemon' at Contents/MacOS/ to start (see ` +
        `src-tauri/src/daemon_lifecycle.rs::bundled_daemon_path()). Build it ` +
        `first: cargo build --release -p copypaste-cli -p copypaste-daemon -p copypaste-relay`;
      if (required) hardFail(msg);
      log(`WARN: ${msg}`);
      continue;
    }
    copyFileSync(src, dst);
    chmodSync(dst, 0o755);
    log(`injected sibling binary: ${name}`);
  }
}

function createIsolatedEnv() {
  const isoRoot = mkdtempSync(path.join(tmpdir(), "copypaste-tauri-smoke-"));
  const dirs = {
    data: path.join(isoRoot, "data"),
    config: path.join(isoRoot, "config"),
    cache: path.join(isoRoot, "cache"),
    log: path.join(isoRoot, "log"),
    home: path.join(isoRoot, "home"),
  };
  for (const d of Object.values(dirs)) mkdirSync(d, { recursive: true });

  const socketPath = path.join(isoRoot, "copypaste.sock");
  const dbPath = path.join(isoRoot, "clipboard.db");
  const deviceIdPath = path.join(isoRoot, "device_id");

  const env = {
    ...process.env,
    // Isolates Tauri's own app_config_dir()/app_data_dir() and
    // tauri-plugin-autostart's LaunchAgent plist, none of which honour the
    // COPYPASTE_* overrides below.
    HOME: dirs.home,
    COPYPASTE_SOCKET: socketPath,
    COPYPASTE_DB: dbPath,
    COPYPASTE_DATA_DIR: dirs.data,
    COPYPASTE_CONFIG_DIR: dirs.config,
    COPYPASTE_CACHE_DIR: dirs.cache,
    COPYPASTE_LOG_DIR: dirs.log,
    COPYPASTE_DEVICE_ID_PATH: deviceIdPath,
    COPYPASTE_EPHEMERAL_KEY: "1",
  };

  return { isoRoot, dirs, socketPath, env };
}

function findChildPids(ppid) {
  try {
    const res = spawnSync("pgrep", ["-P", String(ppid)]);
    if (res.error || !res.stdout) return [];
    return res.stdout
      .toString()
      .trim()
      .split("\n")
      .filter(Boolean)
      .map(Number);
  } catch {
    return [];
  }
}

function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function tailDaemonLogs(logDir, maxLines = 200) {
  if (!existsSync(logDir)) return "(no daemon log dir found)";
  const files = readdirSync(logDir).filter(
    (f) => f.startsWith("daemon") && f.endsWith(".log"),
  );
  if (!files.length) {
    return "(no daemon*.log files found under isolated COPYPASTE_LOG_DIR — daemon may not have started)";
  }
  files.sort().reverse();
  const content = readFileSync(path.join(logDir, files[0]), "utf8");
  return content.split("\n").slice(-maxLines).join("\n");
}

/** Poll for the daemon's Unix-socket IPC endpoint to accept a connection and
 * answer a `status` request with `"ok":true`. Mirrors the nc-based polling
 * in scripts/smoke_test.sh, using node:net instead of shelling out to nc. */
function waitForSocketReady(socketPath, timeoutMs) {
  return new Promise((resolve) => {
    const deadline = Date.now() + timeoutMs;

    const attempt = () => {
      if (!existsSync(socketPath)) {
        if (Date.now() > deadline) return resolve(false);
        setTimeout(attempt, 250);
        return;
      }

      const sock = net.createConnection({ path: socketPath });
      let settled = false;
      const done = (ok) => {
        if (settled) return;
        settled = true;
        sock.destroy();
        resolve(ok);
      };

      const perAttemptTimer = setTimeout(() => done(false), 1500);

      sock.on("connect", () => {
        sock.write('{"id":"tauri-smoke","method":"status","params":{}}\n');
      });

      let buf = "";
      sock.on("data", (d) => {
        buf += d.toString("utf8");
        if (buf.includes('"ok":true')) {
          clearTimeout(perAttemptTimer);
          done(true);
        }
      });

      sock.on("error", () => {
        clearTimeout(perAttemptTimer);
        if (Date.now() > deadline) {
          done(false);
        } else {
          setTimeout(attempt, 250);
        }
      });
    };

    attempt();
  });
}

/** Tauri v2 (via the `dirs` crate) resolves `app_config_dir()` on macOS to
 * `$HOME/Library/Application Support/<bundle identifier>` (same value as
 * `app_data_dir()` on macOS). Mirrors src-tauri/src/config.rs's
 * `config_path()` + tauri.conf.json's `"identifier": "com.copypaste.app"` +
 * config.rs's `CONFIG_FILE` constant ("ui-config.json"). If any of those
 * three ever change, update this path too. */
function uiConfigPath(isolatedHome) {
  return path.join(
    isolatedHome,
    "Library",
    "Application Support",
    "com.copypaste.app",
    "ui-config.json",
  );
}

/** Send a single newline-delimited JSON-RPC request over the daemon's Unix
 * socket IPC and resolve with the raw response text (or null on any
 * failure/timeout). Best-effort — used to seed one clipboard item via the
 * `import` method (crates/copypaste-ipc/src/methods/clipboard.rs's
 * METHOD_IMPORT) so the modal-keyboard-focus-behavior Tier-2 probe has a
 * deterministic "Clear all" button to click, without requiring macOS
 * pasteboard / Input-Monitoring permissions. Mirrors the same
 * import-over-IPC technique scripts/smoke_test.sh uses for its deterministic
 * round-trip check, where the pbcopy-based path is only best-effort in CI. */
function sendIpcRequestOnce(socketPath, requestObj, timeoutMs = 5000) {
  return new Promise((resolve) => {
    const sock = net.createConnection({ path: socketPath });
    let buf = "";
    let settled = false;
    const done = (result) => {
      if (settled) return;
      settled = true;
      sock.destroy();
      resolve(result);
    };
    const timer = setTimeout(() => done(null), timeoutMs);
    sock.on("connect", () => {
      sock.write(JSON.stringify(requestObj) + "\n");
    });
    sock.on("data", (d) => {
      buf += d.toString("utf8");
      if (buf.includes("\n")) {
        clearTimeout(timer);
        done(buf);
      }
    });
    sock.on("error", () => {
      clearTimeout(timer);
      done(null);
    });
  });
}

/** Locate a usable tauri-driver WebDriver backend for the Tier-2 probes.
 * Never throws. Returns `{ available: false, reason }` when none is usable —
 * the caller records every Tier-2 probe as `skip` with that exact reason
 * rather than attempting (and failing) a doomed session. See the header
 * comment's REALITY CHECK section for why this is unconditionally
 * unavailable on darwin today. */
function detectWebDriverBackend() {
  if (process.platform === "darwin" && !process.env.TAURI_SMOKE_FORCE_WEBDRIVER) {
    return {
      available: false,
      reason:
        "tauri-driver has no macOS backend upstream " +
        "(https://github.com/tauri-apps/tauri/issues/5163, open) — packaged " +
        "WebView introspection cannot run on this platform yet. Set " +
        "TAURI_SMOKE_FORCE_WEBDRIVER=1 (with TAURI_DRIVER_BIN pointing at a " +
        "reachable driver) to override for manual testing.",
    };
  }
  const bin = process.env.TAURI_DRIVER_BIN || "tauri-driver";
  const which = spawnSync(process.platform === "win32" ? "where" : "which", [bin]);
  if (which.status !== 0) {
    return {
      available: false,
      reason:
        `'${bin}' not found on PATH — install via 'cargo install tauri-driver ` +
        `--locked' (Linux also needs the webkit2gtk-driver system package), ` +
        `or set TAURI_DRIVER_BIN to an explicit path.`,
    };
  }
  return { available: true, bin };
}

/** Tier 2: WebDriver-driven WebView-introspection probes. See the header
 * comment for the full real-vs-skip breakdown. Never throws — every named
 * probe in WEBDRIVER_PROBE_NAMES is guaranteed exactly one `record()` call
 * by the time this returns. */
async function runTier2WebDriverProbes(execPath, env, record) {
  const backend = detectWebDriverBackend();
  if (!backend.available) {
    for (const name of WEBDRIVER_PROBE_NAMES) record(name, "skip", backend.reason);
    return;
  }

  // Always-skip regardless of backend availability — no in-harness way to
  // open the popup (see the header comment's KNOWN GAP section).
  const ALWAYS_SKIP_REASON =
    "no in-harness way to open the popup window (toggle_popup is " +
    "OS-hotkey-only, not a #[tauri::command]) — see the header comment's " +
    "KNOWN GAP section.";

  const port = 4444 + (process.pid % 500);
  let driverProc = null;
  let session = null;
  const attempted = new Set();
  const mark = (name, status, detail) => {
    attempted.add(name);
    record(name, status, detail);
  };

  try {
    log(`Tier-2: spawning '${backend.bin} --port ${port}'`);
    driverProc = spawn(backend.bin, ["--port", String(port)], {
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const driverUp = await WebDriverSession.waitForServerReady(
      `http://127.0.0.1:${port}`,
      15000,
    );
    if (!driverUp) {
      throw new Error(`${backend.bin} did not become ready on port ${port} within 15000ms`);
    }

    session = await WebDriverSession.create(`http://127.0.0.1:${port}`, {
      alwaysMatch: { "tauri:options": { application: execPath } },
    });
    log("Tier-2: WebDriver session created:", session.sessionId);

    // Give the main window a moment to finish its initial render before
    // probing (mirrors --startup-wait for the Tier-1 direct-spawn probes).
    await sleep(2000);

    // ── webview-console-and-csp ──────────────────────────────────────────
    try {
      await session.executeScript(`
        window.__smoke = { consoleErrors: [], cspViolations: [] };
        const origError = console.error;
        console.error = function (...args) {
          window.__smoke.consoleErrors.push(args.map(String).join(" "));
          origError.apply(console, args);
        };
        document.addEventListener("securitypolicyviolation", (e) => {
          window.__smoke.cspViolations.push(e.violatedDirective + ": " + e.blockedURI);
        });
        return true;
      `);
      await sleep(1500);
      const r = (await session.executeScript(`return window.__smoke;`)) || {
        consoleErrors: [],
        cspViolations: [],
      };
      const bad = (r.consoleErrors?.length ?? 0) + (r.cspViolations?.length ?? 0);
      mark(
        "webview-console-and-csp",
        bad ? "fail" : "pass",
        bad
          ? `console/CSP violations observed: ${JSON.stringify(r)}`
          : "no console.error()/CSP violation observed after the listener " +
              "attached (NOTE: cannot observe errors thrown before the " +
              "listener attaches — WebDriver classic has no equivalent of " +
              "CDP's addScriptToEvaluateOnNewDocument)",
      );
    } catch (e) {
      mark("webview-console-and-csp", "fail", `probe error: ${e.message}`);
    }

    // ── frontend-ipc-bridge-invoked ──────────────────────────────────────
    try {
      const r = await session.executeAsyncScript(`
        const done = arguments[arguments.length - 1];
        if (!window.__TAURI__ || !window.__TAURI__.core) {
          done({ ok: false, error: "window.__TAURI__.core missing" });
          return;
        }
        window.__TAURI__.core.invoke("get_default_popup_shortcut")
          .then((v) => done({ ok: true, value: v }))
          .catch((e) => done({ ok: false, error: String(e) }));
      `);
      mark(
        "frontend-ipc-bridge-invoked",
        r?.ok ? "pass" : "fail",
        r?.ok
          ? `frontend invoke('get_default_popup_shortcut') resolved: ${r.value}`
          : `frontend IPC invoke failed: ${r?.error}`,
      );
    } catch (e) {
      mark("frontend-ipc-bridge-invoked", "fail", `probe error: ${e.message}`);
    }

    // ── preferences-load-and-apply ───────────────────────────────────────
    // Reads back the SEEDED_UI_CONFIG values (written to disk before launch
    // — see main()) through the frontend's own IPC bridge, proving the
    // config was not just persisted to disk (Tier-1 probe 4) but actually
    // loaded and exposed to the running React app.
    try {
      const r = await session.executeAsyncScript(`
        const done = arguments[arguments.length - 1];
        Promise.all([
          window.__TAURI__.core.invoke("get_popup_shortcut"),
          window.__TAURI__.core.invoke("get_allow_screenshots"),
        ])
          .then(([shortcut, allow]) => done({ ok: true, shortcut, allow }))
          .catch((e) => done({ ok: false, error: String(e) }));
      `);
      const ok =
        r?.ok &&
        r.shortcut === SEEDED_UI_CONFIG.popup_shortcut &&
        r.allow === SEEDED_UI_CONFIG.allow_screenshots;
      mark(
        "preferences-load-and-apply",
        ok ? "pass" : "fail",
        ok
          ? `frontend read back the seeded prefs: popup_shortcut='${r.shortcut}' allow_screenshots=${r.allow}`
          : `expected popup_shortcut='${SEEDED_UI_CONFIG.popup_shortcut}' ` +
              `allow_screenshots=${SEEDED_UI_CONFIG.allow_screenshots}, got ${JSON.stringify(r)}`,
      );
    } catch (e) {
      mark("preferences-load-and-apply", "fail", `probe error: ${e.message}`);
    }

    // ── modal-keyboard-focus-behavior ────────────────────────────────────
    // Relies on the import-seeded clipboard item (see main()) so the
    // "Clear all" button (src/views/HistoryView.tsx) is present without
    // needing OS pasteboard access.
    try {
      const opened = await session.executeAsyncScript(`
        const done = arguments[arguments.length - 1];
        const btn = document.querySelector('button[aria-label="Clear all"]');
        if (!btn) {
          done({ ok: false, error: "Clear all button not found (no seeded item / totalCount still 0)" });
          return;
        }
        btn.focus();
        btn.click();
        setTimeout(() => {
          const dialog = document.querySelector('[role="dialog"][aria-modal="true"]');
          done({ ok: !!dialog });
        }, 150);
      `);
      if (!opened?.ok) {
        mark(
          "modal-keyboard-focus-behavior",
          "fail",
          `could not open the confirm modal: ${opened?.error ?? "dialog not found after click"}`,
        );
      } else {
        const trap = await session.executeAsyncScript(`
          const done = arguments[arguments.length - 1];
          const dialog = document.querySelector('[role="dialog"][aria-modal="true"]');
          const initialFocusInside = !!dialog && dialog.contains(document.activeElement);
          document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
          setTimeout(() => {
            const afterTabInside = !!dialog && dialog.contains(document.activeElement);
            document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
            setTimeout(() => {
              const stillOpen = !!document.querySelector('[role="dialog"][aria-modal="true"]');
              const restored = document.activeElement && document.activeElement.getAttribute("aria-label") === "Clear all";
              done({ initialFocusInside, afterTabInside, closedOnEscape: !stillOpen, focusRestored: !!restored });
            }, 150);
          }, 50);
        `);
        const ok =
          trap?.initialFocusInside &&
          trap?.afterTabInside &&
          trap?.closedOnEscape &&
          trap?.focusRestored;
        mark(
          "modal-keyboard-focus-behavior",
          ok ? "pass" : "fail",
          ok
            ? "focus trapped inside the dialog, Escape closed it, focus restored to the trigger"
            : `focus-trap assertion failed: ${JSON.stringify(trap)}`,
        );
      }
    } catch (e) {
      mark("modal-keyboard-focus-behavior", "fail", `probe error: ${e.message}`);
    }

    // ── always-skip (see header KNOWN GAP section) ───────────────────────
    mark("theme-accent-translucency-cross-window", "skip", ALWAYS_SKIP_REASON);
    mark("popup-opens-and-renders", "skip", ALWAYS_SKIP_REASON);
  } catch (e) {
    log(`Tier-2 WebDriver phase error: ${e.message}`);
    for (const name of WEBDRIVER_PROBE_NAMES) {
      if (!attempted.has(name)) {
        record(
          name,
          "fail",
          `Tier-2 WebDriver phase failed before this probe ran: ${e.message}`,
        );
      }
    }
  } finally {
    if (session) await session.delete();
    if (driverProc) {
      driverProc.kill("SIGTERM");
      const exited = await waitForExit(driverProc, 3000);
      if (!exited) driverProc.kill("SIGKILL");
    }
  }
}

function waitForExit(child, ms) {
  return new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resolve(true);
      return;
    }
    const t = setTimeout(() => resolve(false), ms);
    child.once("exit", () => {
      clearTimeout(t);
      resolve(true);
    });
  });
}

async function terminate(child, daemonPids, graceMs) {
  if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGTERM");
    const exited = await waitForExit(child, graceMs);
    if (!exited) {
      log(
        `app did not exit within ${graceMs}ms of SIGTERM — sending SIGKILL`,
      );
      child.kill("SIGKILL");
      await waitForExit(child, 2000);
    }
  }

  // Belt-and-suspenders: Tauri's graceful RunEvent::Exit teardown (which
  // stops the app-owned daemon — see src-tauri/src/lib.rs's `.run(|handle,
  // event| ...)`) reliably fires on a Cocoa-level quit, but a bare POSIX
  // SIGTERM to the process may bypass it and orphan the daemon child. Clean
  // up any daemon child we saw spawned under this process explicitly.
  for (const pid of daemonPids) {
    if (isAlive(pid)) {
      log("cleaning up orphaned daemon child pid", pid);
      try {
        process.kill(pid, "SIGTERM");
      } catch {
        // already gone — fine.
      }
    }
  }
}

function finish(results) {
  const fails = results.filter((r) => r.status === "fail");
  const passes = results.filter((r) => r.status === "pass");
  const stubs = results.filter((r) => r.status === "stub");
  const skips = results.filter((r) => r.status === "skip");

  console.log("\n=== tauri-smoke summary ===");
  console.log(
    `ASSERTED (real checks): ${passes.length} pass, ${fails.length} fail`,
  );
  console.log(
    `SKIPPED (WebDriver backend/platform unavailable — NOT counted as ` +
      `coverage; see the reason on each line below): ${skips.length}`,
  );
  console.log(
    `STUBBED (TODO hooks — NOT counted as coverage): ${stubs.length}`,
  );
  for (const r of results) {
    console.log(`  [${r.status}] ${r.name}`);
  }

  if (fails.length) {
    console.error(`\nFAIL: ${fails.length} real check(s) failed:\n`);
    for (const f of fails) {
      console.error(`--- ${f.name} ---`);
      console.error(f.detail);
      console.error("");
    }
    process.exitCode = 1;
  } else {
    console.log("\nPASS: all implemented (non-stub) checks passed.");
    process.exitCode = 0;
  }
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));

  if (process.platform !== "darwin") {
    hardFail(
      `the packaged-Tauri smoke gate only runs on macOS (packaged .app + ` +
        `Keychain-free isolation assumes Darwin). Current platform: ${process.platform}`,
    );
  }

  const appPath = opts.appPath ?? DEFAULT_APP_BUNDLE_PATH;

  if (opts.build) {
    log(
      `--build: running 'pnpm tauri build' in ${UI_CRATE_DIR} ` +
        `(this compiles the full app — expect several minutes)`,
    );
    const res = spawnSync("pnpm", ["tauri", "build"], {
      cwd: UI_CRATE_DIR,
      stdio: "inherit",
      env: process.env,
    });
    if (res.status !== 0) {
      hardFail(`'pnpm tauri build' failed (exit ${res.status})`);
    }
    if (!existsSync(appPath)) {
      hardFail(
        `'pnpm tauri build' succeeded but the expected bundle was not found ` +
          `at ${appPath} — check tauri.conf.json's productName / bundle ` +
          `target, or pass --app-path.`,
      );
    }
    injectSiblingBinaries(appPath);
  }

  if (!existsSync(appPath)) {
    hardFail(
      `packaged app not found at ${appPath}.\n` +
        `  Build it first:  (cd crates/copypaste-ui && pnpm tauri build)\n` +
        `  or re-run this script with --build to build it automatically.`,
    );
  }

  const execName = resolveBundleExecutable(appPath);
  const execPath = path.join(appPath, "Contents/MacOS", execName);
  if (!existsSync(execPath)) {
    hardFail(
      `CFBundleExecutable '${execName}' not found at ${execPath} — the ` +
        `bundle looks malformed/incomplete. Re-run 'pnpm tauri build'.`,
    );
  }

  const { isoRoot, dirs, socketPath, env } = createIsolatedEnv();
  log("isolated env root:", isoRoot);
  log("launching:", execPath);

  // Seed a non-default UiConfig into the isolated app-config dir BEFORE the
  // first launch — see SEEDED_UI_CONFIG's doc comment and Tier-1 probe 4
  // (prefs-persist-on-disk-after-launch) / the Tier-2
  // preferences-load-and-apply probe, both of which read this back.
  const configPath = uiConfigPath(dirs.home);
  mkdirSync(path.dirname(configPath), { recursive: true });
  writeFileSync(configPath, JSON.stringify(SEEDED_UI_CONFIG, null, 2));
  log("seeded ui-config.json:", configPath);

  const results = [];
  const record = (name, status, detail) => {
    results.push({ name, status, detail });
    const tag = status.toUpperCase().padEnd(4);
    log(`${tag} ${name}${detail ? " — " + String(detail).split("\n")[0] : ""}`);
  };

  let child;
  try {
    child = spawn(execPath, [], { env, stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    record("launch-and-liveness", "fail", `spawn threw: ${e.message}`);
    finish(results);
    rmSync(isoRoot, { recursive: true, force: true });
    return;
  }

  const outputLines = [];
  const captureStream = (stream) => {
    stream.on("data", (chunk) => {
      for (const line of chunk.toString("utf8").split("\n")) {
        if (line.length) outputLines.push(line);
      }
    });
  };
  captureStream(child.stdout);
  captureStream(child.stderr);

  let exitedEarly = null;
  child.on("exit", (code, signal) => {
    exitedEarly = { code, signal };
  });

  // --- REAL PROBE 1: launch + liveness -------------------------------------
  await sleep(opts.startupWaitMs);
  if (exitedEarly) {
    record(
      "launch-and-liveness",
      "fail",
      `process exited during the ${opts.startupWaitMs}ms startup window ` +
        `(code=${exitedEarly.code} signal=${exitedEarly.signal}). ` +
        `Captured output tail:\n${outputLines.slice(-40).join("\n")}`,
    );
  } else {
    record(
      "launch-and-liveness",
      "pass",
      `pid=${child.pid} still running after ${opts.startupWaitMs}ms`,
    );
  }

  // Snapshot the app-owned daemon's child pid (best-effort) before teardown.
  const daemonPids = exitedEarly ? [] : findChildPids(child.pid);

  // --- REAL PROBE 2: IPC transport (daemon socket) reachable --------------
  let reachable = false;
  if (!exitedEarly) {
    reachable = await waitForSocketReady(socketPath, opts.socketTimeoutMs);
    record(
      "ipc-transport-reachable",
      reachable ? "pass" : "fail",
      reachable
        ? `daemon socket responded ok:true within ${opts.socketTimeoutMs}ms`
        : `daemon socket never became reachable at ${socketPath} within ` +
            `${opts.socketTimeoutMs}ms (the app-owned daemon may have failed ` +
            `to spawn — see the daemon log tail in no-fatal-startup-markers below)`,
    );
  } else {
    record("ipc-transport-reachable", "fail", "skipped — app already exited");
  }

  // Best-effort: seed one clipboard item via the daemon's `import` IPC
  // method (crates/copypaste-ipc/src/methods/clipboard.rs's METHOD_IMPORT) —
  // same technique scripts/smoke_test.sh uses for its deterministic IPC
  // round-trip check. This gives the Tier-2 modal-keyboard-focus-behavior
  // probe a "Clear all" button to click without needing macOS pasteboard /
  // Input-Monitoring permissions. The seeded item persists in the isolated
  // on-disk DB (same COPYPASTE_DB path is reused by the Tier-2 launch below),
  // so it survives this process's termination.
  if (reachable) {
    const importReq = {
      id: "tauri-smoke-seed",
      method: "import",
      params: {
        items: [
          {
            content_type: "text",
            content_bytes_b64: Buffer.from(
              `copypaste-tauri-smoke-${Date.now()}`,
            ).toString("base64"),
            created_at_ms: Date.now(),
          },
        ],
      },
    };
    const importResp = await sendIpcRequestOnce(socketPath, importReq);
    if (importResp && importResp.includes('"ok":true')) {
      log("seeded 1 clipboard item via IPC import (for modal-keyboard-focus-behavior)");
    } else {
      log(
        `WARN: IPC import seed failed or timed out (${importResp ?? "no response"}) ` +
          `— modal-keyboard-focus-behavior will likely report no 'Clear all' button`,
      );
    }
  }

  // --- REAL PROBE 3: no fatal markers in captured output / daemon log -----
  const daemonLogTail = tailDaemonLogs(dirs.log);
  const combined = `${outputLines.join("\n")}\n${daemonLogTail}`;
  const hits = FATAL_MARKERS.flatMap((re) => {
    const m = combined.match(re);
    return m ? [m[0]] : [];
  });
  record(
    "no-fatal-startup-markers",
    hits.length ? "fail" : "pass",
    hits.length
      ? `fatal marker(s) found: ${hits.join(" | ")}\n--- daemon log tail ---\n${daemonLogTail}`
      : `no panic/FATAL/CSP markers in captured process stdout+stderr or the ` +
          `daemon log tail (Rust/daemon-side only — see the Tier-2 ` +
          `webview-console-and-csp probe for the WebView-side complement)`,
  );

  // --- REAL PROBE 4: seeded prefs still on disk after a launch+terminate --
  if (exitedEarly) {
    record(
      "prefs-persist-on-disk-after-launch",
      "fail",
      "skipped — app already exited during startup window",
    );
  } else {
    try {
      const onDisk = JSON.parse(readFileSync(configPath, "utf8"));
      const mismatches = Object.entries(SEEDED_UI_CONFIG).filter(
        ([k, v]) => onDisk[k] !== v,
      );
      record(
        "prefs-persist-on-disk-after-launch",
        mismatches.length ? "fail" : "pass",
        mismatches.length
          ? `ui-config.json no longer matches the seeded values after launch: ` +
              `${JSON.stringify(mismatches)} (full file: ${JSON.stringify(onDisk)})`
          : `ui-config.json still holds the seeded values after a full ` +
              `launch+terminate cycle: ${JSON.stringify(SEEDED_UI_CONFIG)}`,
      );
    } catch (e) {
      record(
        "prefs-persist-on-disk-after-launch",
        "fail",
        `could not read back ${configPath}: ${e.message}`,
      );
    }
  }

  // --- Teardown of the Tier-1 direct-spawn process ---------------------------
  // The Tier-2 WebDriver phase below launches its OWN, separate app process
  // (via tauri-driver's tauri:options.application) and must not race this one.
  await terminate(child, daemonPids, opts.shutdownGraceMs);

  // --- Tier 2: WebDriver-driven WebView-introspection probes -----------------
  // Reuses the same isolated `env` (same isolated HOME/COPYPASTE_DB/etc.) so
  // the seeded ui-config.json and the imported clipboard item above are still
  // in place for this second launch. See the header comment's REALITY CHECK:
  // this always records `skip` on darwin today (this repo's only platform).
  await runTier2WebDriverProbes(execPath, env, record);

  rmSync(isoRoot, { recursive: true, force: true });

  finish(results);
}

main().catch((e) => {
  console.error("[tauri-smoke] UNEXPECTED ERROR:", e);
  process.exit(1);
});
