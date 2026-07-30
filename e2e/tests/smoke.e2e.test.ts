import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: ["smoke one", "smoke two"] });
}, 240_000);

afterAll(async () => {
  await app?.stop();
});

test("the real WebView runs the real app against the real daemon", async () => {
  const { browser } = app;

  const list = await browser.$('[role="list"][aria-label="Clipboard history"]');
  await list.waitForExist({ timeout: 60_000 });

  const rows = await browser.$$('[role="listitem"]');
  expect(rows.length).toBeGreaterThan(0);

  const text = await browser.execute(() => document.body.innerText);
  expect(text).toContain("smoke two");
});
