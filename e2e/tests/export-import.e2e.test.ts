/** Export/import keeps clipboard text intact through the native format. */
import { writeFileSync } from "node:fs";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { openHistorySearch, waitForRows } from "../src/harness/ui.js";

const ORDINARY = "an ordinary clipping to export";
const ARBITRARY = "api_key=abc123; password=plain-text";

interface ExportData {
  items: Array<{ content: string }>;
  skipped_non_text: number;
  skipped_undecryptable: number;
}

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: [ORDINARY, ARBITRARY] });
  await waitForRows(app.browser, 2);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

describe("export", () => {
  test("includes arbitrary clipboard text", async () => {
    const data = await app.daemon.json<ExportData>(["export"]);

    expect(data.items.map((item) => item.content)).toEqual(
      expect.arrayContaining([ORDINARY, ARBITRARY]),
    );
    expect(data.skipped_non_text).toBe(0);
    expect(data.skipped_undecryptable).toBe(0);
  });
});

describe("import", () => {
  test("preserves arbitrary text without classification", async () => {
    const file = path.join(app.daemon.dataHome, "edited-backup.json");
    writeFileSync(
      file,
      JSON.stringify({
        items: [{ content: ARBITRARY, content_type: "text", created_at: Date.now(), pinned: false }],
        skipped_non_text: 0,
        skipped_undecryptable: 0,
      }),
    );

    const result = await app.daemon.json<{ inserted: number }>(["import", file]);
    expect(result.inserted).toBe(1);
    expect((await app.daemon.items()).find((item) => item.content === ARBITRARY)).toBeDefined();
  });

  test("shows and searches imported arbitrary text", async () => {
    await waitForRows(app.browser, 3, 45_000);
    const search = await openHistorySearch(app.browser);
    await search.setValue("api_key=abc123");
    await app.browser.waitUntil(
      async () => (await app.browser.getPageSource()).includes(ARBITRARY),
      { timeout: 20_000, interval: 500 },
    );
    await search.clearValue();
  });
});
