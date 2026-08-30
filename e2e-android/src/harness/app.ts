import puppeteer, { type Browser, type Page } from "puppeteer-core";

import { appPid, logcatDump, sleep } from "./adb.js";
import {
  APP_ORIGIN,
  finalAttachDiagnostic,
  isAppTarget,
  nextAttachStep,
  type AttachFinalDiagnostic,
  type AttachPagesDiagnostic,
  type AttachRawDiagnostic,
  type PageAutoAttachOutcome,
  webviewComplaints,
} from "./attach.js";
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

/**
 * Raw CDP target from `/json/list`, used to detect the API 34 flat-page
 * attachment case after Puppeteer's `pages()` omits the otherwise valid app
 * page.
 */
interface RawTarget {
  type: string;
  url: string;
  title: string;
  webSocketDebuggerUrl: string;
}

interface RawTargetResult {
  status: "ok" | "http-error" | "fetch-error" | "invalid-json";
  targets: RawTarget[];
}

function rawDiagnostic(result: RawTargetResult): AttachRawDiagnostic {
  const targetTypeHistogram = { page: 0, webview: 0, other: 0 };
  for (const target of result.targets) {
    if (target.type === "page") targetTypeHistogram.page += 1;
    else if (target.type === "webview") targetTypeHistogram.webview += 1;
    else targetTypeHistogram.other += 1;
  }
  return {
    status: result.status,
    count: result.targets.length,
    targetTypeHistogram,
    appOriginMatchCount: result.targets.filter((target) => isAppTarget(target.url)).length,
    webSocketPresent: result.targets.some((target) => Boolean(target.webSocketDebuggerUrl)),
  };
}

async function rawTargets(browserUrl: string): Promise<RawTargetResult> {
  try {
    const response = await fetch(`${browserUrl}/json/list`, { signal: AbortSignal.timeout(3_000) });
    if (!response.ok) return { status: "http-error", targets: [] };
    try {
      const body: unknown = await response.json();
      return Array.isArray(body)
        ? { status: "ok", targets: body as RawTarget[] }
        : { status: "invalid-json", targets: [] };
    } catch {
      return { status: "invalid-json", targets: [] };
    }
  } catch {
    return { status: "fetch-error", targets: [] };
  }
}

class AttachResolutionError extends Error {
  constructor(message: string, readonly diagnostic: AttachFinalDiagnostic) {
    super(message);
  }
}

const PAGE_AUTO_ATTACH = {
  autoAttach: true,
  waitForDebuggerOnStart: true,
  flatten: true,
  filter: [{ type: "tab", exclude: true }, {}],
};

export interface PublicRootConnection {
  send(
    method: "Target.setAutoAttach",
    params: typeof PAGE_AUTO_ATTACH,
    options: { timeout: number },
  ): Promise<unknown>;
}

export interface PublicBrowserSession {
  connection(): PublicRootConnection | undefined;
  detach(): Promise<void>;
}

export interface PublicBrowserTarget {
  createCDPSession(): Promise<PublicBrowserSession>;
}

export interface PublicBrowser {
  target(): PublicBrowserTarget | undefined;
}

class AttachDeadlineExceeded extends Error {}

async function withinRemainingDeadline<T>(
  msLeft: () => number,
  start: () => Promise<T>,
  onLateResult?: (result: T) => void,
): Promise<T> {
  const remaining = msLeft();
  if (remaining <= 0) throw new AttachDeadlineExceeded();

  const operation = Promise.resolve().then(start);
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<T>((_, reject) => {
        timer = setTimeout(() => reject(new AttachDeadlineExceeded()), remaining);
      }),
    ]);
  } catch (error) {
    if (error instanceof AttachDeadlineExceeded && onLateResult) {
      void operation.then(onLateResult).catch(() => undefined);
    }
    throw error;
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

async function detachWithinRemainingDeadline(session: PublicBrowserSession, msLeft: () => number): Promise<boolean> {
  const detach = Promise.resolve().then(() => session.detach());
  if (msLeft() <= 0) {
    void detach.catch(() => undefined);
    return false;
  }
  try {
    await withinRemainingDeadline(msLeft, () => detach);
    return true;
  } catch {
    return false;
  }
}

/**
 * API 34 exposes the app page in `/json/list` but Puppeteer's initial target
 * filter can omit it. Configure the root connection once and let Puppeteer's
 * TargetManager receive the resulting page attachment.
 */
export async function enablePageAutoAttach(
  browser: PublicBrowser,
  msLeft: () => number,
): Promise<PageAutoAttachOutcome> {
  if (msLeft() <= 0) return "deadline-exceeded";

  let target: PublicBrowserTarget | undefined;
  try {
    target = browser.target();
  } catch {
    return "browser-target-unavailable";
  }
  if (!target) return "browser-target-unavailable";

  let session: PublicBrowserSession;
  try {
    session = await withinRemainingDeadline(
      msLeft,
      () => target.createCDPSession(),
      (lateSession) => { void lateSession.detach().catch(() => undefined); },
    );
  } catch (error) {
    return error instanceof AttachDeadlineExceeded ? "deadline-exceeded" : "browser-session-unavailable";
  }

  let outcome: PageAutoAttachOutcome;
  try {
    const rootConnection = session.connection();
    if (!rootConnection) {
      outcome = "root-connection-unavailable";
    } else {
      const timeout = msLeft();
      if (timeout <= 0) {
        outcome = "deadline-exceeded";
      } else {
        await withinRemainingDeadline(msLeft, () =>
          rootConnection.send("Target.setAutoAttach", PAGE_AUTO_ATTACH, { timeout }),
        );
        outcome = "page-autoattach-enabled";
      }
    }
  } catch (error) {
    outcome = error instanceof AttachDeadlineExceeded ? "deadline-exceeded" : "page-autoattach-rejected";
  }

  return (await detachWithinRemainingDeadline(session, msLeft))
    ? outcome
    : "browser-session-detach-failed";
}

async function resolveAppPage(port: number, deadline: number): Promise<Attached> {
  const msLeft = () => Math.max(0, deadline - Date.now());
  let { browser, endpoint } = await open(port, msLeft());
  let autoAttachAttempted = false;
  let pageAutoAttachOutcome: PageAutoAttachOutcome = "not-attempted";
  let diagnostic: AttachFinalDiagnostic = finalAttachDiagnostic(
    { status: "error", count: 0, appOriginMatchCount: 0 },
    {
      status: "fetch-error",
      count: 0,
      targetTypeHistogram: { page: 0, webview: 0, other: 0 },
      appOriginMatchCount: 0,
      webSocketPresent: false,
    },
    "not-attempted",
  );

  for (;;) {
    const pageResult = await browser.pages(true).then(
      (found) => ({ status: "ok" as const, pages: found }),
      () => ({ status: "error" as const, pages: [] as Page[] }),
    );
    const pages = pageResult.pages;
    const page = pages?.find((candidate) => isAppTarget(candidate.url()));
    if (page) return { browser, page, endpoint };

    const rawResult = await rawTargets(endpoint.browserUrl);
    const targets = rawResult.targets;
    const raw = targets.find((t) => isAppTarget(t.url));
    const defaultPageCount = raw
      ? await withinRemainingDeadline(msLeft, () => browser.pages()).then(
        (found) => found.length,
        () => undefined,
      )
      : undefined;
    if (pageResult.status === "ok" && defaultPageCount === 0 && raw && !autoAttachAttempted) {
      autoAttachAttempted = true;
      pageAutoAttachOutcome = await enablePageAutoAttach(browser, msLeft);
    }

    const pagesDiagnostic: AttachPagesDiagnostic = {
      status: pageResult.status,
      count: pages.length,
      appOriginMatchCount: pages.filter((candidate) => isAppTarget(candidate.url())).length,
    };
    diagnostic = finalAttachDiagnostic(
      pagesDiagnostic,
      rawDiagnostic(rawResult),
      pageAutoAttachOutcome,
    );
    const allTargets = pages.map((c) => c.url()).concat(targets.map((t) => t.url));
    const step = nextAttachStep({
      targets: allTargets,
      pid: await appPid(),
      endpointPid: endpoint.pid,
      msLeft: msLeft(),
    });
    if (step.do === "wait") {
      await sleep(500);
      continue;
    }
    await browser.disconnect().catch(() => undefined);
    if (step.do === "give-up") throw new AttachResolutionError(step.why, diagnostic);
    ({ browser, endpoint } = await open(port, msLeft()));
    autoAttachAttempted = false;
    pageAutoAttachOutcome = "not-attempted";
  }
}

/** Best effort by contract: this runs while attachment has already failed, and
 *  an error raised here would replace the reason with this function's. */
async function explained(error: unknown, waitedMs: number): Promise<Error> {
  const reason = error instanceof Error ? error.message : String(error);
  const waited = `${Math.round(waitedMs / 1000)}s`;
  const complaints = webviewComplaints(await logcatDump().catch(() => ""));
  const diagnostic = error instanceof AttachResolutionError ? error.diagnostic : undefined;
  writeAttachFailure({ waited, origin: APP_ORIGIN, reason, complaints, diagnostic });
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
