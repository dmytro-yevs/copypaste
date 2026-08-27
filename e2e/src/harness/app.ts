import path from "node:path";

import { execa } from "execa";
import { remote } from "webdriverio";

import { snapshotAndClearClipboard } from "./clipboard.js";
import {
  DEV_SERVER_URL,
  NATIVE_DRIVER,
  appBinary,
  freePort,
  requireDisplay,
  runLogPath,
} from "./env.js";
import { startDaemon, type Daemon } from "./daemon.js";
import { sleep, track, type Child } from "./process.js";
import { assertReallyRunning, type Browser } from "./webview-guard.js";
import { dismissFirstRun } from "./ui.js";

export interface App {
  readonly browser: Browser;
  readonly daemon: Daemon;
  stop(): Promise<void>;
}

export interface StartOptions {
  /** Items are seeded before the window opens, so the first paint already has them. */
  seed?: readonly string[];
  /** How long to wait for the WebDriver session. Only the guard test, which
   *  wants a failure, has a reason to shorten it. */
  sessionTimeoutMs?: number;
}

/**
 * DMY-54, run 31379514744: the job's first app launch aborted at 60s while the
 * next file opened its session in seconds. A first WebView2 start pays for a new
 * user-data directory and a Defender scan of a freshly built unsigned binary, so
 * the budget must cover one cold start. It stays finite because a wrong binary
 * fails by timing out, and `connectionRetryCount` stays 0 because a retry here
 * would hide a product crash behind a second attempt.
 */
const COLD_SESSION_BUDGET_MS = 120_000;
const SESSION_CLOSE_BUDGET_MS = 10_000;

/**
 * No GPU or software-rendering flags are set here, and none are needed:
 * WebKitGTK 2.52 on this host runs JavaScript and computes layout under plain
 * Xvfb. `LIBGL_ALWAYS_SOFTWARE`, `WEBKIT_DISABLE_COMPOSITING_MODE` and
 * `WEBKIT_DISABLE_DMABUF_RENDERER` were all unnecessary — the EGL DRI3 warnings
 * on stderr are cosmetic. Do not add them speculatively.
 */
export async function startApp(options: StartOptions = {}): Promise<App> {
  requireDisplay();
  await requireDevServer();

  const clipboard = await snapshotAndClearClipboard();
  let daemon: Daemon;
  try {
    daemon = await startDaemon();
  } catch (error) {
    await clipboard.restore();
    throw error;
  }
  try {
    if (options.seed?.length) await daemon.addMany(options.seed);
  } catch (error) {
    try {
      await daemon.stop();
    } finally {
      await clipboard.restore();
    }
    throw error;
  }

  const driverPort = await freePort();
  const nativePort = await freePort();

  // tauri-driver 2.0.6 takes no pass-through for the native driver's own log
  // flags, and it drops msedgedriver's stdout while forwarding its stderr
  // (verified against a stub driver). This file is therefore the only surviving
  // record of an Edge WebDriver that dies during startup.
  const driverLogPath = runLogPath(`${path.basename(daemon.dataHome)}-driver.log`);
  const driver = track(
    execa(
      "tauri-driver",
      [
        "--port",
        String(driverPort),
        "--native-port",
        String(nativePort),
        "--native-driver",
        NATIVE_DRIVER,
      ],
      {
        // tauri-driver spawns the app as a child, so its data-directory and
        // endpoint overrides must be inherited here with the daemon's.
        env: { ...process.env, ...daemon.env },
        stdio: ["ignore", "pipe", "pipe"],
        reject: false,
        killDescendants: true,
      },
    ),
    driverLogPath,
  );

  try {
    await waitForDriver(driverPort, driver);
  } catch (error) {
    try {
      await shutdown(driver, daemon, [driverPort, nativePort]);
    } finally {
      await clipboard.restore();
    }
    throw error;
  }

  const sessionBudgetMs = options.sessionTimeoutMs ?? COLD_SESSION_BUDGET_MS;
  const sessionStarted = Date.now();
  let browser: Browser;
  try {
    browser = await remote({
      hostname: "127.0.0.1",
      port: driverPort,
      path: "/",
      automationProtocol: "webdriver",
      logLevel: "error",
      connectionRetryCount: 0,
      connectionRetryTimeout: sessionBudgetMs,
      capabilities: {
        // @ts-expect-error tauri-driver's vendor capability is not in the W3C types.
        "tauri:options": { application: appBinary() },
      },
    });
  } catch (cause) {
    try {
      await shutdown(driver, daemon, [driverPort, nativePort]);
    } finally {
      await clipboard.restore();
    }
    throw new Error(
      describeSessionFailure({
        budgetMs: sessionBudgetMs,
        elapsedMs: Date.now() - sessionStarted,
        driverState: driver.diagnostics(),
        driverLog: driver.log(),
        logPath: driverLogPath,
      }),
      { cause },
    );
  }

  const app: App = {
    browser,
    daemon,
    stop: (() => {
      let stopPromise: Promise<void> | undefined;
      return () => {
        if (stopPromise) return stopPromise;
        stopPromise = (async () => {
          // Every WebDriver call goes through the page's main thread, so a
          // frozen app makes a polite close hang as surely as the probe does.
          await Promise.race([
            browser.deleteSession().catch(() => undefined),
            sleep(SESSION_CLOSE_BUDGET_MS),
          ]);
          try {
            await shutdown(driver, daemon, [driverPort, nativePort]);
          } finally {
            await clipboard.restore();
          }
        })();
        return stopPromise;
      };
    })(),
  };

  try {
    await assertReallyRunning(browser, driver);
    await dismissFirstRun(browser);
  } catch (error) {
    await app.stop();
    throw error;
  }

  return app;
}

/**
 * A debug build loads `devUrl`, so a dead dev server produces a WebKit error
 * page — a real, queryable document that mounts nothing. The probe then reports
 * "the app never mounted a UI", which sends whoever reads it looking at the
 * app. Checking here names the actual cause, which in a shared container is
 * usually another process reclaiming port 1420.
 */
async function requireDevServer(): Promise<void> {
  try {
    const response = await fetch(DEV_SERVER_URL, { signal: AbortSignal.timeout(5_000) });
    if (response.ok) return;
    throw new Error(`HTTP ${response.status}`);
  } catch (cause) {
    throw new Error(
      `the Vite dev server on ${DEV_SERVER_URL} is not answering, so the app ` +
        `would load an error page instead of the UI. Global setup started one, ` +
        `so something outside this run took the port or killed it.`,
      { cause },
    );
  }
}

async function waitForDriver(port: number, driver: Child): Promise<void> {
  const deadline = Date.now() + 30_000;
  for (;;) {
    if (driver.exited()) {
      throw new Error(`tauri-driver exited during startup:\n${driver.log()}`);
    }
    try {
      const response = await fetch(`http://127.0.0.1:${port}/status`, {
        signal: AbortSignal.timeout(2_000),
      });
      if (response.ok || response.status === 404) return;
    } catch {
      /* not listening yet */
    }
    if (Date.now() > deadline) {
      throw new Error(`tauri-driver never listened on ${port}:\n${driver.log()}`);
    }
    await sleep(200);
  }
}

async function shutdown(
  driver: Child,
  daemon: Daemon,
  ports: readonly number[] = [],
): Promise<void> {
  try {
    await driver.stop();
    await waitForPortsClosed(ports);
  } finally {
    await daemon.stop();
  }
}

async function waitForPortsClosed(ports: readonly number[]): Promise<void> {
  if (ports.length === 0) return;
  const deadline = Date.now() + SESSION_CLOSE_BUDGET_MS;
  for (;;) {
    const open = await Promise.all(
      ports.map(async (port) => {
        try {
          await fetch(`http://127.0.0.1:${port}/status`, {
            signal: AbortSignal.timeout(500),
          });
          return true;
        } catch {
          return false;
        }
      }),
    );
    if (!open.some(Boolean)) return;
    if (Date.now() >= deadline) {
      throw new Error(
        `WebDriver ports remain open after ${SESSION_CLOSE_BUDGET_MS}ms: ` +
          `${ports.join(", ")}`,
      );
    }
    await sleep(100);
  }
}

/**
 * `hyper::Error(IncompleteMessage)` is what tauri-driver logs when webdriverio
 * abandons its own `POST /session` at the budget — the echo of this client
 * giving up, not a fault. DMY-54 was first read the other way round, so the
 * elapsed time and the budget are stated before the log is quoted.
 */
export function describeSessionFailure(context: {
  budgetMs: number;
  elapsedMs: number;
  driverState: string;
  driverLog: string;
  logPath: string;
}): string {
  return (
    `could not open a WebDriver session against the app: gave up after ` +
    `${context.elapsedMs}ms of a ${context.budgetMs}ms budget. ` +
    `tauri-driver ${context.driverState}, full output at ${context.logPath}.\n` +
    `${context.driverLog.trim() || "<no output captured>"}`
  );
}
