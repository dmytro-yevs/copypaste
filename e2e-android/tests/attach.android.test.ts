import { afterAll, expect, test } from "vitest";

import { PACKAGE } from "../src/harness/adb.js";
import { attachToApp, type AndroidApp } from "../src/harness/app.js";
import { beforeAllWithEvidence } from "../src/harness/suite.js";
import { NAV } from "../src/harness/ui.js";

let app: AndroidApp;

beforeAllWithEvidence("attach", async () => {
  app = await attachToApp();
}, 180_000);

afterAll(async () => {
  await app?.detach();
});

test("the endpoint is this app's WebView, not the shared zygote's", () => {
  expect(app.endpoint.version["Android-Package"]).toBe(PACKAGE);
  expect(app.endpoint.socket).toBe(`webview_devtools_remote_${app.endpoint.pid}`);
});

test("the engine is the Android system WebView", () => {
  // Recorded, not pinned: which Chromium a device carries is the device's
  // business. What matters is that it is not WebKitGTK.
  const agent = app.endpoint.version["User-Agent"] ?? "";
  expect(agent).toMatch(/Android/);
  expect(agent).toMatch(/\bwv\b/);
  console.log(
    `Android WebView ${app.endpoint.version["Browser"]} · V8 ${app.endpoint.version["V8-Version"]}`,
  );
});

test("the document is the app's, and it answers a DOM query", async () => {
  expect(app.page.url()).toContain("tauri.localhost");
  expect(await app.page.title()).toBe("CopyPaste");
  console.log(`title=${await app.page.title()} url=${app.page.url()}`);

  const nav = await app.withPage((page) =>
    page.evaluate(
      (selector) =>
        Array.from(document.querySelectorAll(`${selector} button`)).map(
          (node) => node.textContent?.trim() ?? "",
        ),
      NAV,
    ),
  );
  expect(nav).toEqual(expect.arrayContaining(["Library", "Devices", "Settings"]));
});

test("the frontend mounted rather than merely painting", async () => {
  // A blank WebView also has a body and a title. A React root with children is
  // what android-smoke.sh's `assert_painted` cannot tell apart from one.
  const mounted = await app.withPage((page) =>
    page.evaluate(() => (document.querySelector("#root")?.childElementCount ?? 0) > 0),
  );
  expect(mounted).toBe(true);
});
