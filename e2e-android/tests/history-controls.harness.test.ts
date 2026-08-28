import { describe, expect, test } from "vitest";

import {
  controlledMenuReached,
  sameSortedItemIds,
  sortedItemIds,
  withCleanupPreservingPrimary,
} from "../src/harness/history-controls.js";

describe("History filter harness contracts", () => {
  test("rejects a restored list with an unexpected extra row", () => {
    const expected = sortedItemIds(["row-b", "row-a"]);
    expect(sameSortedItemIds(expected, ["row-a", "row-b"])).toBe(true);
    expect(sameSortedItemIds(expected, ["row-a", "row-b", "row-c"])).toBe(
      false,
    );
  });

  test("keeps the primary failure first when cleanup also fails", async () => {
    const primary = new Error("primary interaction failed");
    const cleanup = new Error("portal cleanup failed");
    let caught: unknown;
    try {
      await withCleanupPreservingPrimary(
        async () => {
          throw primary;
        },
        async () => {
          throw cleanup;
        },
      );
    } catch (error) {
      caught = error;
    }

    expect(caught).toBeInstanceOf(AggregateError);
    expect((caught as AggregateError).errors).toEqual([primary, cleanup]);
  });

  test("does not accept an unexpanded trigger with stale state or an orphan portal", () => {
    expect(
      controlledMenuReached(
        {
          ariaExpanded: "false",
          triggerState: "open",
          menuPresent: true,
          menuRole: "menu",
          menuState: "open",
        },
        false,
      ),
    ).toBe(false);
    expect(
      controlledMenuReached(
        {
          ariaExpanded: "false",
          triggerState: "closed",
          menuPresent: false,
          menuRole: null,
          menuState: null,
        },
        false,
      ),
    ).toBe(true);
  });
});
