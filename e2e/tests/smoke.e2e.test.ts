import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { rowCount } from "../src/harness/ui.js";

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: ["smoke one", "smoke two"] });
  // 300s like every other app-starting hook: file order is not fixed, so this
  // one can be the cold first launch, and a hook that expires before the
  // session budget replaces a precise diagnostic with "Hook timed out".
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

test("the real WebView runs the real app against the real daemon", async () => {
  const { browser } = app;

  const list = await browser.$('[role="list"][aria-label="Clipboard history"]');
  await list.waitForExist({ timeout: 60_000 });

  expect(await rowCount(browser)).toBeGreaterThan(0);

  const text = await browser.execute(() => document.body.innerText);
  expect(text).toContain("smoke two");
});
