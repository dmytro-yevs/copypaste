import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import {
  accessibleSurface,
  expectNoFilesystemPath,
  expectNoRawError,
  outerHtml,
} from "../src/harness/leaks.js";
import { visibleText, waitForRows } from "../src/harness/ui.js";

let app: App;
let dataHome: string;

beforeAll(async () => {
  app = await startApp({ seed: ["something to show first"] });
  dataHome = app.daemon.dataHome;
  await waitForRows(app.browser, 1);

  // The failure under test is the transport one: the socket path is what
  // discloses the local username, and it is only in play once the daemon dies
  // under a running window.
  await app.daemon.stop();

  await app.browser.waitUntil(
    async () => (await visibleText(app.browser)).includes("service"),
    {
      timeout: 40_000,
      interval: 500,
      timeoutMsg: "the UI never reported the service as unreachable",
    },
  );
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

test("the offline state is reported to the user", async () => {
  const text = await visibleText(app.browser);
  expect(text.toLowerCase()).toMatch(/service/);
});

test("no user-facing string contains a filesystem path (INV-12)", async () => {
  expectNoFilesystemPath(await accessibleSurface(app.browser), dataHome);
});

test("the raw transport error is not rendered anywhere", async () => {
  expectNoRawError(await outerHtml(app.browser));
});
