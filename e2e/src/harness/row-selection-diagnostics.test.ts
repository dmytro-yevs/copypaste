import { describe, expect, test, vi } from "vitest";

import type { Browser } from "./webview-guard.js";
import { captureRowSelectionClick } from "./row-selection-diagnostics.js";

describe("row selection diagnostics", () => {
  test("preserves a click failure when page reads fail and still disarms", async () => {
    const clickFailure = new Error("native row click failed");
    const execute = vi
      .fn()
      .mockResolvedValueOnce(1)
      .mockRejectedValueOnce(new Error("document unavailable before click"))
      .mockRejectedValueOnce(new Error("document unavailable after click"))
      .mockResolvedValueOnce(true);
    const browser = { execute } as unknown as Browser;

    let thrown: unknown;
    try {
      await captureRowSelectionClick(
        browser,
        "item-2",
        async () => {
          throw clickFailure;
        },
        { intendedIds: ["item-1", "item-2"] },
      );
    } catch (cause) {
      thrown = cause;
    }

    expect(thrown).toBeInstanceOf(Error);
    expect((thrown as Error).cause).toBe(clickFailure);
    expect((thrown as Error).message).toContain('"reason":"read-failed"');
    expect((thrown as Error).message).not.toContain("document unavailable");
    expect(execute).toHaveBeenCalledTimes(4);
  });
});
