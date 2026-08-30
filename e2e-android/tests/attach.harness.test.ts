import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

import {
  finalAttachDiagnostic,
  isAppTarget,
  nextAttachStep,
  webviewComplaints,
  type AttachSample,
} from "../src/harness/attach.js";
import {
  enablePageAutoAttach,
  resolvePageAutoAttach,
  type PublicBrowser,
  type PublicBrowserSession,
  type PublicRootConnection,
} from "../src/harness/app.js";

const RUNNING: AttachSample = {
  targets: [],
  pid: 3761,
  endpointPid: 3761,
  msLeft: 60_000,
};

describe("waiting for the app's page target", () => {
  // Run 31671766432, API 33 and 34: the WebView answered /json/version with no
  // page target yet and the harness gave up 30s in, eight seconds before the
  // next leg found the same app navigable.
  test("keeps waiting while the app it resolved is still the app that is running", () => {
    expect(nextAttachStep(RUNNING)).toEqual({ do: "wait" });
    expect(nextAttachStep({ ...RUNNING, targets: ["about:blank"] })).toEqual({ do: "wait" });
  });

  test("attaches to the app's target and not to another one the WebView exposes", () => {
    expect(isAppTarget("http://tauri.localhost/index.html")).toBe(true);
    expect(isAppTarget("http://tauri.localhost/")).toBe(true);
    // The default port is the origin's port, so spelling it out is the same page.
    expect(isAppTarget("http://tauri.localhost:80/")).toBe(true);
    expect(isAppTarget("about:blank")).toBe(false);
    expect(isAppTarget("")).toBe(false);
  });

  // A substring match attached to any of these and then drove it as the app.
  test("does not attach to a host that merely contains the origin", () => {
    for (const impostor of [
      "https://tauri.localhost.example.test/",
      "https://not-tauri.localhost/",
      "https://example.test/?next=http://tauri.localhost/",
    ]) {
      expect(isAppTarget(impostor)).toBe(false);
    }
  });

  // A host match accepted all of these: the same name is a different document
  // under another scheme or port, and the harness drove it as the app.
  test("does not attach to the app's host under another scheme or port", () => {
    for (const elsewhere of [
      "https://tauri.localhost/",
      "http://tauri.localhost:444/",
      "https://tauri.localhost:443/",
      "ftp://tauri.localhost/",
      "file://tauri.localhost/index.html",
      "tauri://tauri.localhost/",
    ]) {
      expect(isAppTarget(elsewhere)).toBe(false);
    }
  });
});

describe("recovering from a lifecycle transition", () => {
  // The socket name carries the pid, so the forward outlives the process it was
  // resolved for and answers nothing.
  test("re-resolves the endpoint when the app restarted under it", () => {
    expect(nextAttachStep({ ...RUNNING, pid: 4102 })).toEqual({
      do: "reopen",
      why: "the app restarted (pid 3761 → 4102)",
    });
  });

  test("re-resolves when the process is gone and time is left to find it again", () => {
    expect(nextAttachStep({ ...RUNNING, pid: undefined })).toEqual({
      do: "reopen",
      why: "the app process (pid 3761) is gone",
    });
  });

  test("re-resolves when the devtools connection stopped answering", () => {
    expect(nextAttachStep({ ...RUNNING, targets: undefined })).toEqual({
      do: "reopen",
      why: "pid 3761 is running but its devtools connection stopped answering",
    });
  });
});

// DMY-141: WebView 109 (API 33) exposes targets with type "webview"; Puppeteer
// 24's pages() excludes them unless includeAll is true.
test("requests all target types so WebView pages are not excluded", () => {
  const here = fileURLToPath(import.meta.url);
  const app = readFileSync(resolve(here, "../../src/harness/app.ts"), "utf8");
  expect(app).toMatch(/\.pages\(\s*true\s*\)/);
});

// DMY-141: On old WebView engines (109, API 29/33) Puppeteer's `pages(true)`
// omits the app target. The ground truth is `/json/list` — the raw CDP endpoint
// that lists every open debugger page. The harness must fall back to it when
// `pages()` yields no matching target.
test("falls back to /json/list when pages() hides the app target", () => {
  const here = fileURLToPath(import.meta.url);
  const app = readFileSync(resolve(here, "../../src/harness/app.ts"), "utf8");
  expect(app).toMatch(/\/json\/list/);
  expect(app).toMatch(/rawTargets/);
});

// The ground-truth query must be bounded: a devtools endpoint that never
// answers `/json/list` must not delay attachment beyond its normal backoff.
test("raw target discovery bounds its fetch", () => {
  const here = fileURLToPath(import.meta.url);
  const app = readFileSync(resolve(here, "../../src/harness/app.ts"), "utf8");
  expect(app).toMatch(/AbortSignal\.timeout/);
});

describe("giving up", () => {
  test("names the process state rather than the wait when the app died", () => {
    expect(nextAttachStep({ ...RUNNING, pid: undefined, msLeft: 0 })).toEqual({
      do: "give-up",
      why: "the app process (pid 3761) is gone",
    });
  });

  test("distinguishes a WebView with no target from one with other targets", () => {
    expect(nextAttachStep({ ...RUNNING, msLeft: 0 })).toEqual({
      do: "give-up",
      why: "pid 3761 is running and its WebView exposes no page target at all",
    });
    expect(nextAttachStep({ ...RUNNING, msLeft: 0, targets: ["about:blank"] })).toEqual({
      do: "give-up",
      why: "pid 3761 exposes 1 page target(s)",
    });
  });

  // The API 29 leg's whole diagnosis was one line in another leg's logcat: the
  // WebView 74 that image carries cannot parse the shipped bundle.
  test("reports what the device said, and reports it only once", () => {
    const logcat = [
      "08-13 05:57:10.638 E chromium: [ERROR:filesystem_posix.cc(89)] stat Crashpad: No such file",
      "08-13 05:57:13.300 E Tauri/Console: File: http://tauri.localhost/assets/index.js - Line 2 - Msg: Uncaught SyntaxError: Unexpected token =",
      "08-13 05:57:13.300 E Tauri/Console: File: http://tauri.localhost/assets/index.js - Line 2 - Msg: Uncaught SyntaxError: Unexpected token =",
      "08-13 05:57:25.262 I am_proc_start: [0,5190,10133,com.android.chrome,service]",
    ].join("\n");

    expect(webviewComplaints(logcat)).toEqual([
      "08-13 05:57:13.300 E Tauri/Console: File: http://tauri.localhost/assets/index.js - Line 2 - Msg: Uncaught SyntaxError: Unexpected token =",
    ]);
  });

  test("keeps a crash, and keeps the last complaints rather than the first", () => {
    const logcat = ["Render process 1", "FATAL EXCEPTION 2", "E AndroidRuntime 3"].join("\n");

    expect(webviewComplaints(logcat, 2)).toEqual(["FATAL EXCEPTION 2", "E AndroidRuntime 3"]);
    expect(webviewComplaints("nothing interesting here")).toEqual([]);
  });
});

describe("bounded final attach diagnostics", () => {
  test("uses a raw app target when pages() is empty", () => {
    expect(
      finalAttachDiagnostic(
        {
          status: "ok",
          count: 0,
          appOriginMatchCount: 0,
        },
        {
          status: "ok",
          count: 1,
          targetTypeHistogram: { page: 0, webview: 1, other: 0 },
          appOriginMatchCount: 1,
          webSocketPresent: true,
        },
        "page-autoattach-enabled",
        "confirmed",
      ),
    ).toEqual({
      pages: { status: "ok", count: 0, appOriginMatchCount: 0 },
      raw: {
        status: "ok",
        count: 1,
        targetTypeHistogram: { page: 0, webview: 1, other: 0 },
        appOriginMatchCount: 1,
        webSocketPresent: true,
      },
      pageAutoAttachOutcome: "page-autoattach-enabled",
      pageAutoAttachCleanup: "confirmed",
    });
  });

  test("distinguishes a raw target-list error from an empty list", () => {
    const pages = { status: "error" as const, count: 0, appOriginMatchCount: 0 };
    const rawError = { status: "fetch-error" as const, count: 0, targetTypeHistogram: { page: 0, webview: 0, other: 0 }, appOriginMatchCount: 0, webSocketPresent: false };
    const rawEmpty = { ...rawError, status: "ok" as const };
    const error = finalAttachDiagnostic(pages, rawError, "page-autoattach-rejected", "confirmed");
    const empty = finalAttachDiagnostic(pages, rawEmpty, "not-attempted", "not-needed");
    expect(error.raw.status).toBe("fetch-error");
    expect(empty.raw.status).toBe("ok");
    expect(JSON.stringify(error)).not.toMatch(/https?:|title|provider|secret/i);
  });

  test("counts unknown target types as other without retaining target values", () => {
    const diagnostic = finalAttachDiagnostic(
      {
        status: "ok", count: 1, appOriginMatchCount: 0,
      },
      {
        status: "ok", count: 1,
        targetTypeHistogram: { page: 0, webview: 0, other: 1 },
        appOriginMatchCount: 0, webSocketPresent: true,
      },
      "not-attempted",
      "not-needed",
    );
    expect(diagnostic).toEqual({
      pages: { status: "ok", count: 1, appOriginMatchCount: 0 },
      raw: {
        status: "ok", count: 1,
        targetTypeHistogram: { page: 0, webview: 0, other: 1 },
        appOriginMatchCount: 0, webSocketPresent: true,
      },
      pageAutoAttachOutcome: "not-attempted",
      pageAutoAttachCleanup: "not-needed",
    });
  });
});

function publicBrowser(
  session: PublicBrowserSession,
  events: string[],
): PublicBrowser {
  return {
    target: () => ({
      createCDPSession: async () => {
        events.push("create-session");
        return session;
      },
    }),
    disconnect: async () => { events.push("disconnect-browser"); },
  };
}

describe("public root CDP auto-attach", () => {
  test("uses the exact page-safe filter once and detaches its temporary session", async () => {
    const events: string[] = [];
    const root: PublicRootConnection = {
      send: async (method, params, options) => {
        events.push("set-auto-attach");
        expect(method).toBe("Target.setAutoAttach");
        expect(params).toEqual({
          autoAttach: true,
          waitForDebuggerOnStart: true,
          flatten: true,
          filter: [{ type: "tab", exclude: true }, {}],
        });
        expect(options).toEqual({ timeout: 137 });
      },
    };
    const session: PublicBrowserSession = {
      connection: () => root,
      detach: async () => { events.push("detach-session"); },
    };

    await expect(enablePageAutoAttach(publicBrowser(session, events), () => 137))
      .resolves.toEqual({ outcome: "page-autoattach-enabled", cleanup: "confirmed" });
    expect(events).toEqual(["create-session", "set-auto-attach", "detach-session"]);
  });

  test("reports temporary-session cleanup failure without exposing a provider error", async () => {
    const root: PublicRootConnection = { send: async () => undefined };
    const session: PublicBrowserSession = {
      connection: () => root,
      detach: async () => { throw new Error("provider error https://secret.test"); },
    };

    await expect(enablePageAutoAttach(publicBrowser(session, []), () => 100))
      .resolves.toEqual({ outcome: "page-autoattach-enabled", cleanup: "unconfirmed" });
  });

  test("does not begin a public CDP operation after the attachment budget expires", async () => {
    const events: string[] = [];
    const session: PublicBrowserSession = {
      connection: () => undefined,
      detach: async () => { events.push("detach-session"); },
    };

    await expect(enablePageAutoAttach(publicBrowser(session, events), () => 0))
      .resolves.toEqual({ outcome: "deadline-exceeded", cleanup: "not-needed" });
    expect(events).toEqual([]);

    const expiringEvents: string[] = [];
    const root: PublicRootConnection = {
      send: async () => { expiringEvents.push("set-auto-attach"); },
    };
    const expiringSession: PublicBrowserSession = {
      connection: () => root,
      detach: async () => { expiringEvents.push("detach-session"); },
    };
    let remainingCalls = 0;
    const remaining = () => (++remainingCalls === 5 ? 0 : 100);

    await expect(enablePageAutoAttach(publicBrowser(expiringSession, expiringEvents), remaining))
      .resolves.toEqual({ outcome: "page-autoattach-enabled", cleanup: "unconfirmed" });
    await Promise.resolve();
    expect(expiringEvents).toEqual(["create-session", "set-auto-attach", "detach-session"]);
  });

  test("reports an unavailable browser target without creating a session", async () => {
    const browser: PublicBrowser = { target: () => undefined, disconnect: async () => undefined };

    await expect(enablePageAutoAttach(browser, () => 100))
      .resolves.toEqual({ outcome: "browser-target-unavailable", cleanup: "not-needed" });
  });

  test("reports a rejected browser session without a provider error", async () => {
    const browser: PublicBrowser = {
      target: () => ({ createCDPSession: async () => { throw new Error("provider error"); } }),
      disconnect: async () => undefined,
    };

    await expect(enablePageAutoAttach(browser, () => 100))
      .resolves.toEqual({ outcome: "browser-session-unavailable", cleanup: "not-needed" });
  });

  test("detaches a temporary session whose root connection is unavailable", async () => {
    const events: string[] = [];
    const session: PublicBrowserSession = {
      connection: () => undefined,
      detach: async () => { events.push("detach-session"); },
    };

    await expect(enablePageAutoAttach(publicBrowser(session, events), () => 100))
      .resolves.toEqual({ outcome: "root-connection-unavailable", cleanup: "confirmed" });
    expect(events).toEqual(["create-session", "detach-session"]);
  });

  test("rejects the auto-attach request and still detaches its temporary session", async () => {
    const events: string[] = [];
    const root: PublicRootConnection = {
      send: async () => { throw new Error("rejected"); },
    };
    const session: PublicBrowserSession = {
      connection: () => root,
      detach: async () => { events.push("detach-session"); },
    };

    await expect(enablePageAutoAttach(publicBrowser(session, events), () => 100))
      .resolves.toEqual({ outcome: "page-autoattach-rejected", cleanup: "confirmed" });
    expect(events).toEqual(["create-session", "detach-session"]);
  });

  test("gates the resolver on the owned page and attempts once per browser", async () => {
    const candidate = {
      pagesStatus: "ok" as const,
      defaultPageCount: 0,
      rawTarget: { type: "page", url: "http://tauri.localhost/" },
      currentPid: 3761,
      endpointPid: 3761,
    };
    const session: PublicBrowserSession = {
      connection: () => ({ send: async () => undefined }),
      detach: async () => undefined,
    };
    const state = { attempted: false, outcome: "not-attempted" as const, cleanup: "not-needed" as const };
    const events: string[] = [];
    const browser = publicBrowser(session, events);

    const first = await resolvePageAutoAttach(browser, state, candidate, () => 100);
    const second = await resolvePageAutoAttach(browser, first.state, candidate, () => 100);
    expect(first.blocked).toBe(false);
    expect(second.state).toBe(first.state);
    expect(events).toEqual(["create-session"]);

    const newBrowserEvents: string[] = [];
    await resolvePageAutoAttach(publicBrowser(session, newBrowserEvents), state, candidate, () => 100);
    expect(newBrowserEvents).toEqual(["create-session"]);

    for (const rejected of [
      { ...candidate, rawTarget: { type: "webview", url: "http://tauri.localhost/" } },
      { ...candidate, rawTarget: { type: "page", url: "https://tauri.localhost/" } },
      { ...candidate, defaultPageCount: 1 },
      { ...candidate, currentPid: 4102 },
    ]) {
      const rejectedEvents: string[] = [];
      const result = await resolvePageAutoAttach(publicBrowser(session, rejectedEvents), state, rejected, () => 100);
      expect(result).toEqual({ state, blocked: false });
      expect(rejectedEvents).toEqual([]);
    }
  });

  test("blocks the resolver and disconnects when temporary-session cleanup is unconfirmed", async () => {
    const events: string[] = [];
    const session: PublicBrowserSession = {
      connection: () => ({ send: async () => undefined }),
      detach: async () => {
        events.push("detach-session");
        throw new Error("cleanup failed");
      },
    };
    const result = await resolvePageAutoAttach(
      publicBrowser(session, events),
      { attempted: false, outcome: "not-attempted", cleanup: "not-needed" },
      {
        pagesStatus: "ok",
        defaultPageCount: 0,
        rawTarget: { type: "page", url: "http://tauri.localhost/" },
        currentPid: 3761,
        endpointPid: 3761,
      },
      () => 100,
    );

    expect(result).toMatchObject({
      blocked: true,
      state: { outcome: "page-autoattach-enabled", cleanup: "unconfirmed" },
    });
    expect(result).not.toHaveProperty("page");
    expect(events).toEqual(["create-session", "detach-session", "disconnect-browser"]);
  });

  test("has no private Puppeteer or direct-page connection dependency", () => {
    const here = fileURLToPath(import.meta.url);
    const app = readFileSync(resolve(here, "../../src/harness/app.ts"), "utf8");
    expect(app).toMatch(/browser\.target\(\).*createCDPSession/s);
    expect(app).toMatch(/session\.connection\(\)/);
    expect(app).toMatch(/target\.type === "page" && isAppTarget\(target\.url\)/);
    expect(app).not.toMatch(/browserWSEndpoint|directAppTarget|_targetManager|_connection/);
  });
});
