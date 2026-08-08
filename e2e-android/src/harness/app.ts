import puppeteer, { type Browser, type Page } from "puppeteer-core";

import { sleep } from "./adb.js";
import { DEFAULT_PORT, openDevtools, type DevtoolsEndpoint } from "./devtools.js";

export type { Page } from "puppeteer-core";

/** Tauri serves the frontend from its own protocol; nothing else in the app has a page target. */
const APP_ORIGIN = "tauri.localhost";

/**
 * Android may destroy and recreate the activity — a configuration change, a
 * process death under memory pressure, a relaunch — and the WebView goes with
 * it. CDP then answers every call with "detached Frame" or "Target closed",
 * which says nothing about the app being fine on the far side. Re-resolve and
 * reattach instead of failing the assertion that happened to be in flight.
 */
const GONE = /detached Frame|Target closed|Session closed|Execution context was destroyed|Connection closed/i;

async function connect(port: number): Promise<{
  browser: Browser;
  page: Page;
  endpoint: DevtoolsEndpoint;
}> {
  const endpoint = await openDevtools(port);
  const browser = await puppeteer.connect({
    browserURL: endpoint.browserUrl,
    defaultViewport: null,
  });

  const deadline = Date.now() + 30_000;
  let seen: string[] = [];
  while (Date.now() < deadline) {
    const pages = await browser.pages();
    seen = pages.map((page) => page.url());
    const page = pages.find((candidate) => candidate.url().includes(APP_ORIGIN));
    if (page) return { browser, page, endpoint };
    await sleep(500);
  }
  await browser.disconnect();
  throw new Error(
    `the WebView exposes no ${APP_ORIGIN} page target; it has ${seen.length ? seen.join(", ") : "none at all"}`,
  );
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
    return new AndroidApp(await connect(port));
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
    const parts = await connect(this.#endpoint.port);
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
