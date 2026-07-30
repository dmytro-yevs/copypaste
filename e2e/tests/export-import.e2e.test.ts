/**
 * Export and import, and what the window shows afterwards.
 *
 * There is no UI for either: `Method::Export` / `Method::Import` exist in the
 * daemon and the CLI drives them, but no Tauri command routes them, so a user
 * of the app cannot back up or restore anything. That is a rule 6 gap, recorded
 * here rather than asserted away — what this file *can* assert is the part that
 * would be a security failure if it were wrong.
 *
 * The two claims:
 *
 *  - An export withholds flagged items by default **and says how many** it
 *    withheld. A silent export that dropped items is worse than one that says
 *    so, and a user who asked for a backup must not get a plaintext file full
 *    of credentials by accident either.
 *  - `is_sensitive` on an imported item is a **floor, never a ceiling**
 *    (manifest 04, PG-26). An edited backup that marks a credential clean must
 *    come back flagged — and the window must render it as withheld, which is
 *    the half no daemon test can see.
 */
import { writeFileSync } from "node:fs";
import path from "node:path";

import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { startApp, type App } from "../src/harness/app.js";
import { expectSecretAbsent, outerHtml } from "../src/harness/leaks.js";
import { visibleText, waitForRows, waitForText } from "../src/harness/ui.js";

const ORDINARY = "an ordinary clipping to export";
const SECRET = "AKIAIOSFODNN7EXAMPLE";
/** Marked clean in the file it is imported from. */
const SMUGGLED = "AKIAIOSFODNN7SMUGGLE";

interface ExportData {
  items: Array<{ content: string; is_sensitive: boolean }>;
  skipped_non_text: number;
  skipped_sensitive: number;
  skipped_undecryptable: number;
}

let app: App;

beforeAll(async () => {
  app = await startApp({ seed: [ORDINARY, SECRET] });
  await waitForRows(app.browser, 2);
}, 300_000);

afterAll(async () => {
  await app?.stop();
});

describe("export", () => {
  test("withholds a flagged item by default, and counts what it withheld", async () => {
    const data = await app.daemon.json<ExportData>(["export"]);

    expect(data.items.map((item) => item.content)).toContain(ORDINARY);
    expect(data.items.map((item) => item.content)).not.toContain(SECRET);
    expect(data.skipped_sensitive).toBe(1);
    // Present even at zero: a count nobody knows to look for is not a count.
    expect(data.skipped_non_text).toBe(0);
    expect(data.skipped_undecryptable).toBe(0);
  });

  test("includes it only when asked, and then says nothing was withheld", async () => {
    const data = await app.daemon.json<ExportData>(["export", "--include-sensitive"]);

    const secret = data.items.find((item) => item.content === SECRET);
    expect(secret, "the credential was not in the opt-in export").toBeDefined();
    expect(secret!.is_sensitive).toBe(true);
    expect(data.skipped_sensitive).toBe(0);
  });
});

describe("import", () => {
  test("re-runs the detector, so a credential cannot arrive marked clean", async () => {
    const file = path.join(app.daemon.dataHome, "edited-backup.json");
    writeFileSync(
      file,
      JSON.stringify({
        items: [
          {
            content: SMUGGLED,
            content_type: "text",
            created_at: Date.now(),
            pinned: false,
            // The lie under test.
            is_sensitive: false,
          },
        ],
        skipped_non_text: 0,
        skipped_sensitive: 0,
        skipped_undecryptable: 0,
      }),
    );

    const result = await app.daemon.json<{ inserted: number; skipped: number }>([
      "import",
      file,
    ]);
    expect(result.inserted).toBe(1);

    const stored = (await app.daemon.items()).find(
      (item) => item.content === SMUGGLED,
    );
    expect(stored, "the imported item is missing").toBeDefined();
    expect(stored!.is_sensitive, "an import overrode the detector").toBe(true);
  });

  test("the window renders the smuggled item as withheld", async () => {
    await waitForRows(app.browser, 3, 45_000);
    await app.browser.waitUntil(
      async () =>
        (await visibleText(app.browser)).split("Sensitive content hidden").length > 2,
      {
        timeout: 45_000,
        interval: 500,
        timeoutMsg: "the imported credential never rendered as a withheld row",
      },
    );

    // Absent from the document, not merely covered: the bridge sends
    // `content: null` for a flagged item, so there is nothing to blur.
    await expectSecretAbsent(app.browser, SMUGGLED);
    expect(await outerHtml(app.browser)).toContain(ORDINARY);
  });

  test("and it never reaches the search index", async () => {
    const search = await app.browser.$('[aria-label="Search clipboard history"]');
    await search.setValue(SMUGGLED.slice(0, 12));
    await waitForText(app.browser, "No results for", 20_000);
    await expectSecretAbsent(app.browser, SMUGGLED);
    await search.clearValue();
  });
});
