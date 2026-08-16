import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

import {
  isAppTarget,
  nextAttachStep,
  webviewComplaints,
  type AttachSample,
} from "../src/harness/attach.js";

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
      why: "pid 3761 exposes about:blank",
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
