import { execa } from "execa";
import { remote } from "webdriverio";

import {
  DEV_SERVER_URL,
  NATIVE_DRIVER,
  appBinary,
  freePort,
  requireDisplay,
} from "./env.js";
import { startDaemon, type Daemon } from "./daemon.js";
import { sleep, track, type Child } from "./process.js";

export type Browser = Awaited<ReturnType<typeof remote>>;

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
 * No GPU or software-rendering flags are set here, and none are needed:
 * WebKitGTK 2.52 on this host runs JavaScript and computes layout under plain
 * Xvfb. `LIBGL_ALWAYS_SOFTWARE`, `WEBKIT_DISABLE_COMPOSITING_MODE` and
 * `WEBKIT_DISABLE_DMABUF_RENDERER` were all unnecessary — the EGL DRI3 warnings
 * on stderr are cosmetic. Do not add them speculatively.
 */
export async function startApp(options: StartOptions = {}): Promise<App> {
  requireDisplay();
  await requireDevServer();

  const daemon = await startDaemon();
  if (options.seed?.length) await daemon.addMany(options.seed);

  const driverPort = await freePort();
  const nativePort = await freePort();

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
        // tauri-driver spawns the app as a child, so the app's view of
        // XDG_DATA_HOME — and therefore which daemon socket it dials — is
        // decided here and nowhere else.
        env: { ...process.env, ...daemon.env },
        stdio: ["ignore", "pipe", "pipe"],
        reject: false,
        detached: true,
      },
    ),
    { group: true },
  );

  await waitForDriver(driverPort, driver);

  let browser: Browser;
  try {
    browser = await remote({
      hostname: "127.0.0.1",
      port: driverPort,
      path: "/",
      automationProtocol: "webdriver",
      logLevel: "error",
      // A real app answers in a few seconds; the budget is generous for a cold
      // cache but finite, because a wrong binary fails by timing out.
      connectionRetryCount: 0,
      connectionRetryTimeout: options.sessionTimeoutMs ?? 60_000,
      capabilities: {
        // @ts-expect-error tauri-driver's vendor capability is not in the W3C types.
        "tauri:options": { application: appBinary() },
      },
    });
  } catch (cause) {
    await shutdown(driver, daemon);
    throw new Error(
      `could not open a WebDriver session against the app:\n${driver.log()}`,
      { cause },
    );
  }

  const app: App = {
    browser,
    daemon,
    async stop() {
      // Every WebDriver call goes through the page's main thread, so a frozen
      // app makes a polite session close hang as surely as the probe does.
      // Killing the driver is what actually reclaims the process.
      await Promise.race([
        browser.deleteSession().catch(() => undefined),
        sleep(10_000),
      ]);
      await shutdown(driver, daemon);
    },
  };

  try {
    await assertReallyRunning(browser, driver);
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

async function shutdown(driver: Child, daemon: Daemon): Promise<void> {
  await driver.stop();
  await daemon.stop();
}

/**
 * The guard against a suite that passes while testing nothing.
 *
 * Every check below has failed at least once during development, and each
 * failure mode is silent: a WebView that loads but runs no JavaScript still
 * answers `execute` with `null`, and a dev server that is down produces a
 * WebKit error page whose DOM is a valid, queryable, entirely wrong document.
 */
async function assertReallyRunning(browser: Browser, driver: Child): Promise<void> {
  const capabilities = browser.capabilities as { browserName?: string };
  if (capabilities.browserName !== "wry") {
    throw new Error(
      `expected the Tauri WebView ("wry"), got "${capabilities.browserName}". ` +
        `The session is not the app under test.`,
    );
  }

  // An app stuck in a render loop never yields its main thread, so `execute`
  // does not return at all — the probe hangs instead of failing. Without this
  // wall clock the whole suite stalls until the CI job is killed, which reads
  // as an infrastructure problem rather than as the app being broken.
  await Promise.race([
    probeUntilMounted(browser, driver),
    new Promise<never>((_, reject) =>
      setTimeout(
        () =>
          reject(
            new Error(
              "the WebView never answered a script within 90s. Its main thread " +
                "is blocked — an infinite render loop does this — so the app is " +
                "running but cannot paint or respond.\n" +
                `tauri-driver log:\n${driver.log()}`,
            ),
          ),
        90_000,
      ),
    ),
  ]);
}

async function probeUntilMounted(browser: Browser, driver: Child): Promise<void> {
  const deadline = Date.now() + 60_000;
  let last = "";
  for (;;) {
    const probe = (await browser.execute(function () {
      const root = document.getElementById("root");
      return {
        js: 2 + 2,
        bridge: "__TAURI_INTERNALS__" in window,
        nodes: document.querySelectorAll("*").length,
        rootChildren: root ? root.childElementCount : -1,
        text: document.body ? document.body.innerText.slice(0, 300) : "",
        url: location.href,
      };
    })) as {
      js: number;
      bridge: boolean;
      nodes: number;
      rootChildren: number;
      text: string;
      url: string;
    } | null;

    if (probe === null) {
      throw new Error(
        "the WebView returned null for a script that cannot return null — " +
          "JavaScript is not executing in the app.",
      );
    }
    if (probe.js !== 4) {
      throw new Error(`the WebView did not evaluate arithmetic: got ${probe.js}`);
    }
    if (!probe.bridge) {
      throw new Error(
        `window.__TAURI_INTERNALS__ is absent at ${probe.url} — the page is ` +
          `loaded outside the Tauri bridge, so no IPC is under test.`,
      );
    }
    if (probe.rootChildren > 0 && probe.nodes > 30) return;

    last = `url=${probe.url} nodes=${probe.nodes} rootChildren=${probe.rootChildren} text=${JSON.stringify(probe.text)}`;
    if (Date.now() > deadline) {
      throw new Error(
        `the app never mounted a UI. Last probe: ${last}\n` +
          `tauri-driver log:\n${driver.log()}`,
      );
    }
    await sleep(250);
  }
}
