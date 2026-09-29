import { afterAll, beforeAll, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { openHistorySearch, waitForRows } from "../src/harness/ui.js";

const ARBITRARY = "api_key=abc123; password=plain-text";
const ORDINARY = "an ordinary clipping";

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: [ORDINARY, ARBITRARY] });
  await waitForRows(app.browser, 2);
}, 300_000);

afterAll(async () => { await app?.stop(); });

test("arbitrary clipboard text is stored and rendered", async () => {
  expect((await app.daemon.items()).find((item) => item.content === ARBITRARY)).toBeDefined();
  const html = (await app.browser.execute(() => document.documentElement.outerHTML)) as string;
  expect(html).toContain(ORDINARY);
  expect(html).toContain(ARBITRARY);
});

test("arbitrary clipboard text is searchable", async () => {
  const search = await openHistorySearch(app.browser);
  await search.setValue("api_key=abc123");
  await app.browser.waitUntil(
    async () => ((await app.browser.execute(() => document.body.innerText)) as string).includes(ARBITRARY),
    { timeout: 15_000, timeoutMsg: "search did not show the matching clipping" },
  );
});
