import puppeteer, { type Browser, type Page } from "puppeteer-core";

import { appPid, logcatDump, sleep } from "./adb.js";
import { APP_ORIGIN, isAppTarget, nextAttachStep, webviewComplaints } from "./attach.js";
import { DEFAULT_PORT, openDevtools, type DevtoolsEndpoint } from "./devtools.js";
import { rememberAttachedApp, writeAttachFailure } from "./evidence.js";

export type { Page } from "puppeteer-core";

/**
 * One budget for resolving the socket and for the page target behind it.
 *
 * A cold start on an emulator is minutes, not seconds: run 31671766432's API 33
 * and 34 legs published a WebView that answered `/json/version` with no page
 * target yet, and the storage leg found the same app navigable eight seconds
 * after the harness had given up on it. The bound stays — an app that never
 * paints must still fail — it is just no longer shorter than a start.
 */
const ATTACH_TIMEOUT_MS = Number(process.env.COPYPASTE_ATTACH_TIMEOUT_MS ?? 150_000);

/**
 * A warm app that lost its WebView comes back in seconds, and the startup
 * budget would outlive the shortest hook timeout in `tests/` — which replaces
 * this harness's reason with vitest's "hook timed out".
 */
const REATTACH_TIMEOUT_MS = 20_000;

/**
 * Android may destroy and recreate the activity — a configuration change, a
 * process death under memory pressure, a relaunch — and the WebView goes with
 * it. CDP then answers every call with "detached Frame" or "Target closed",
 * which says nothing about the app being fine on the far side. Re-resolve and
 * reattach instead of failing the assertion that happened to be in flight.
 */
const GONE = /detached Frame|Target closed|Session closed|Execution context was destroyed|Connection closed/i;

interface Attached {
  browser: Browser;
  page: Page;
  endpoint: DevtoolsEndpoint;
}

async function open(port: number, msLeft: number): Promise<{ browser: Browser; endpoint: DevtoolsEndpoint }> {
  const endpoint = await openDevtools(port, msLeft);
  const browser = await puppeteer.connect({ browserURL: endpoint.browserUrl, defaultViewport: null });
  return { browser, endpoint };
}

async function resolveAppPage(port: number, deadline: number): Promise<Attached> {
  const msLeft = () => Math.max(0, deadline - Date.now());
  let { browser, endpoint } = await open(port, msLeft());

  for (;;) {
    const pages = await browser.pages().then(
      (found) => found,
      () => undefined,
    );
    const page = pages?.find((candidate) => isAppTarget(candidate.url()));
    if (page) return { browser, page, endpoint };

    const step = nextAttachStep({
      targets: pages?.map((candidate) => candidate.url()),
      pid: await appPid(),
      endpointPid: endpoint.pid,
      msLeft: msLeft(),
    });
    if (step.do === "wait") {
      await sleep(500);
      continue;
    }
    await browser.disconnect().catch(() => undefined);
    if (step.do === "give-up") throw new Error(step.why);
    ({ browser, endpoint } = await open(port, msLeft()));
  }
}

/** Best effort by contract: this runs while attachment has already failed, and
 *  an error raised here would replace the reason with this function's. */
async function explained(error: unknown, waitedMs: number): Promise<Error> {
  const reason = error instanceof Error ? error.message : String(error);
  const waited = `${Math.round(waitedMs / 1000)}s`;
  const complaints = webviewComplaints(await logcatDump().catch(() => ""));
  writeAttachFailure({ waited, origin: APP_ORIGIN, reason, complaints });
  return new Error(
    `no ${APP_ORIGIN} page target after ${waited}: ${reason}` +
      (complaints.length ? `. The device said: ${complaints.join(" | ")}` : ""),
  );
}

async function connect(port: number, timeoutMs = ATTACH_TIMEOUT_MS): Promise<Attached> {
  try {
    return await resolveAppPage(port, Date.now() + timeoutMs);
  } catch (error) {
    throw await explained(error, timeoutMs);
  }
}

export class AndroidApp {
  #browser: Browser;
  #page: Page;
  #endpoint: DevtoolsEndpoint;

  private constructor(parts: { browser: Browser; page: Page; endpoint: DevtoolsEndpoint }) {
    this.#browser = parts.browser;
    this.#page = parts.page;
    this.#endpoint = parts.endpoint;
  }

  /**
   * Attach to the WebView of an app that is already running. Deliberately not
   * "start the app": installing and launching belongs to
   * `scripts/release/android-smoke.sh`, and this harness runs against the
   * process that leaves behind.
   */
  static async attach(port = DEFAULT_PORT): Promise<AndroidApp> {
    const app = new AndroidApp(await connect(port));
    rememberAttachedApp(app);
    return app;
  }

  get page(): Page {
    return this.#page;
  }

  get endpoint(): DevtoolsEndpoint {
    return this.#endpoint;
  }

  async withPage<T>(action: (page: Page) => Promise<T>): Promise<T> {
    for (let attempt = 0; ; attempt++) {
      try {
        return await action(this.#page);
      } catch (error) {
        if (attempt >= 2 || !GONE.test(String(error))) throw error;
        await this.#reattach();
      }
    }
  }

  async #reattach(): Promise<void> {
    await this.#browser.disconnect().catch(() => undefined);
    const parts = await connect(this.#endpoint.port, REATTACH_TIMEOUT_MS);
    this.#browser = parts.browser;
    this.#page = parts.page;
    this.#endpoint = parts.endpoint;
  }

  async detach(): Promise<void> {
    await this.#browser.disconnect().catch(() => undefined);
  }
}

export async function attachToApp(port = DEFAULT_PORT): Promise<AndroidApp> {
  return AndroidApp.attach(port);
}
