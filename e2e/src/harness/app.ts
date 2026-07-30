import { execa, type ResultPromise } from "execa";
import { remote } from "webdriverio";

import {
  NATIVE_DRIVER,
  appBinary,
  freePort,
  requireDisplay,
} from "./env.js";
import { startDaemon, type Daemon } from "./daemon.js";

export type Browser = Awaited<ReturnType<typeof remote>>;

export interface App {
  readonly browser: Browser;
  readonly daemon: Daemon;
  stop(): Promise<void>;
}

export interface StartOptions {
  /** Items are seeded before the window opens, so the first paint already has them. */
  seed?: readonly string[];
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

  const daemon = await startDaemon();
  if (options.seed?.length) await daemon.addMany(options.seed);

  const driverPort = await freePort();
  const nativePort = await freePort();

  const driver = execa(
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
      // XDG_DATA_HOME — and therefore which daemon socket it dials — is decided
      // here and nowhere else.
      env: { ...process.env, ...daemon.env },
      stdio: ["ignore", "pipe", "pipe"],
      reject: false,
    },
  );
  const driverLog: string[] = [];
  driver.stdout?.on("data", (c: Buffer) => driverLog.push(c.toString()));
  driver.stderr?.on("data", (c: Buffer) => driverLog.push(c.toString()));

  await waitForDriver(driverPort, driver, driverLog);

  let browser: Browser;
  try {
    browser = await remote({
      hostname: "127.0.0.1",
      port: driverPort,
      path: "/",
      automationProtocol: "webdriver",
      logLevel: "error",
      connectionRetryCount: 1,
      connectionRetryTimeout: 180_000,
      capabilities: {
        // @ts-expect-error tauri-driver's vendor capability is not in the W3C types.
        "tauri:options": { application: appBinary() },
      },
    });
  } catch (cause) {
    await shutdown(driver, daemon);
    throw new Error(
      `could not open a WebDriver session against the app:\n${driverLog.join("")}`,
      { cause },
    );
  }

  const app: App = {
    browser,
    daemon,
    async stop() {
      await browser.deleteSession().catch(() => undefined);
      await shutdown(driver, daemon);
    },
  };

  try {
    await assertReallyRunning(browser, driverLog);
  } catch (error) {
    await app.stop();
    throw error;
  }

  return app;
}

async function waitForDriver(
  port: number,
  driver: ResultPromise,
  log: string[],
): Promise<void> {
  const deadline = Date.now() + 30_000;
  for (;;) {
    if (driver.exitCode !== null && driver.exitCode !== undefined) {
      throw new Error(`tauri-driver exited (${driver.exitCode}):\n${log.join("")}`);
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
      throw new Error(`tauri-driver never listened on ${port}:\n${log.join("")}`);
    }
    await new Promise((r) => setTimeout(r, 200));
  }
}

async function shutdown(driver: ResultPromise, daemon: Daemon): Promise<void> {
  driver.kill("SIGTERM");
  await Promise.race([
    driver.catch(() => undefined),
    new Promise((r) => setTimeout(r, 5_000)),
  ]);
  driver.kill("SIGKILL");
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
async function assertReallyRunning(browser: Browser, driverLog: string[]): Promise<void> {
  const capabilities = browser.capabilities as { browserName?: string };
  if (capabilities.browserName !== "wry") {
    throw new Error(
      `expected the Tauri WebView ("wry"), got "${capabilities.browserName}". ` +
        `The session is not the app under test.`,
    );
  }

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
          `tauri-driver log:\n${driverLog.join("")}`,
      );
    }
    await new Promise((r) => setTimeout(r, 250));
  }
}
