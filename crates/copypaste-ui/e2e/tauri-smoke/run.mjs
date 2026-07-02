#!/usr/bin/env node
/**
 * tauri-smoke/run.mjs — packaged-Tauri smoke gate.
 *
 * design-system-redesign tasks.md 6.17 ("PACKAGED-Tauri smoke/integration
 * checks = the product release gate") + bd CopyPaste-g27b.13.2.
 *
 * This is the PACKAGED-UI complement to `scripts/smoke_test.sh` (which covers
 * daemon/CLI/IPC correctness on a release build). This script instead launches
 * the actual bundled `CopyPaste.app` produced by `pnpm tauri build` and probes
 * it as a black box — no test code runs inside the WebView.
 *
 * ─────────────────────────────────────────────────────────────────────────
 * WHAT IS ASSERTED (real checks — a failure here is loud and exits non-zero):
 * ─────────────────────────────────────────────────────────────────────────
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
 *                                    the `frontend-ipc-bridge-invoked` STUB.
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
 *                                    WebView — this codebase has no
 *                                    devtools/tauri-plugin-log bridge wired up
 *                                    to forward those to process stdio. See
 *                                    the `webview-console-and-csp` STUB below.
 *
 * ─────────────────────────────────────────────────────────────────────────
 * WHAT IS STUBBED (explicit TODO hooks — recorded as `stub`, NEVER fail the
 * gate on their own; they exist so the gap is visible and auditable instead
 * of silently unclaimed):
 * ─────────────────────────────────────────────────────────────────────────
 *   - webview-console-and-csp        — true in-page console.error()/CSP
 *     violation capture. Needs a WebDriver (tauri-driver) attach or a
 *     temporary JS→Rust console forwarder (would touch src/**, out of scope
 *     for this change).
 *   - frontend-ipc-bridge-invoked    — proof the React app itself called
 *     `ipc_call` and got a response (as opposed to probe #2 above, which only
 *     proves the daemon socket accepts connections).
 *   - preferences-load-and-apply     — seed a non-default UiConfig, relaunch,
 *     and read back applied UI state (needs WebView DOM/state introspection).
 *   - theme-accent-translucency-cross-window — correct theme/accent/
 *     translucency on BOTH the main window and the popup, including the
 *     packaged next-open cross-window check (design-system-redesign 1.15/1.17).
 *   - popup-opens-and-renders        — trigger the popup (shortcut or a
 *     tauri-driver command invocation) and confirm the popup window/WebView
 *     reaches a rendered state.
 *   - modal-keyboard-focus-behavior  — focus trap / Escape / backdrop-click /
 *     focus-restoration inside the packaged WebView (can differ from
 *     Chromium, e.g. Tab order, IME composition).
 *
 * Each stub is a distinct, named attachment point in `main()` below — replace
 * `recordStub(...)` with a real `record(...)` call once a WebDriver harness
 * exists, without needing to restructure the rest of the script.
 *
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
 *
 * Exit code: 0 only if every REAL (non-stub) check passed. Non-zero and a
 * clearly labelled FAIL block otherwise.
 */

import { spawn, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  rmSync,
  existsSync,
  readdirSync,
  readFileSync,
  mkdirSync,
  copyFileSync,
  chmodSync,
} from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

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

  console.log("\n=== tauri-smoke summary ===");
  console.log(
    `ASSERTED (real checks): ${passes.length} pass, ${fails.length} fail`,
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

  const results = [];
  const record = (name, status, detail) => {
    results.push({ name, status, detail });
    const tag = status.toUpperCase().padEnd(4);
    log(`${tag} ${name}${detail ? " — " + String(detail).split("\n")[0] : ""}`);
  };
  const recordStub = (name, detail) => record(name, "stub", detail);

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
  if (!exitedEarly) {
    const reachable = await waitForSocketReady(socketPath, opts.socketTimeoutMs);
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
          `daemon log tail (Rust/daemon-side only — see webview-console-and-csp ` +
          `stub for the WebView-side gap)`,
  );

  // --- STUB PROBES: explicit TODO hooks, do not affect the exit code ------
  recordStub(
    "webview-console-and-csp",
    "TODO: WKWebView console.error()/CSP-violation events are not forwarded " +
      "to process stdio by this codebase (no tauri-plugin-log / devtools " +
      "bridge wired up). Attach via tauri-driver + WebDriver (BiDi console " +
      "API), or a temporary JS->Rust console forwarder, then replace this " +
      "stub with a real assertion.",
  );
  recordStub(
    "frontend-ipc-bridge-invoked",
    "TODO: probe #2 above only proves the daemon's Unix socket accepts " +
      "connections. Attach via WebDriver and invoke a window.__TAURI__ " +
      "command from inside the WebView JS context to prove the frontend " +
      "itself completed an ipc_call round-trip.",
  );
  recordStub(
    "preferences-load-and-apply",
    "TODO: seed a known non-default UiConfig into the isolated config dir " +
      "before launch, then read back the applied DOM/state (e.g. data-theme, " +
      "popup shortcut display) via WebDriver to confirm it was loaded and " +
      "applied.",
  );
  recordStub(
    "theme-accent-translucency-cross-window",
    "TODO: attach via WebDriver to both the main window and the popup " +
      "WebView, read computed styles / data-theme,data-accent attributes, " +
      "then close+reopen the popup (packaged next-open cross-window check " +
      "per design-system-redesign 1.15/1.17) and diff before/after.",
  );
  recordStub(
    "popup-opens-and-renders",
    "TODO: trigger the popup shortcut (or invoke the toggle_popup Tauri " +
      "command via tauri-driver) and assert the popup window becomes " +
      "visible and its WebView reaches a rendered/ready state.",
  );
  recordStub(
    "modal-keyboard-focus-behavior",
    "TODO: via WebDriver, open a modal in the packaged WebView and assert " +
      "focus trap, Escape-to-close, backdrop-click-to-close, and " +
      "focus-restoration to the trigger element — packaged WKWebView " +
      "keyboard handling can differ from Chromium (e.g. Tab order, IME).",
  );

  // --- Teardown -------------------------------------------------------------
  await terminate(child, daemonPids, opts.shutdownGraceMs);
  rmSync(isoRoot, { recursive: true, force: true });

  finish(results);
}

main().catch((e) => {
  console.error("[tauri-smoke] UNEXPECTED ERROR:", e);
  process.exit(1);
});
