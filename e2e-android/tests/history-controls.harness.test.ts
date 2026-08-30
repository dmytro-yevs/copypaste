import { describe, expect, test } from "vitest";

import {
  controlledMenuReached,
  historyFilterDiagnostic,
  normalizeAriaChecked,
  safeDiagnosticIds,
  safeJSON,
  sameSortedItemIds,
  sortedItemIds,
  withCleanupPreservingPrimary,
} from "../src/harness/history-controls.js";

const ITEM_ID = "123e4567-e89b-42d3-a456-426614174000";

function listSnapshotWith(overrides: Partial<Parameters<typeof historyFilterDiagnostic>[4]> = {}) {
  return {
    scrollTop: 0,
    scrollHeight: 720,
    clientHeight: 407,
    totalSize: 720,
    frame: 1818,
    rows: [],
    ...overrides,
  };
}

describe("History filter harness contracts", () => {
  test("bounds and redacts hostile diagnostic ids", () => {
    const result = safeDiagnosticIds([
      ITEM_ID,
      "bad id\nwith text",
      "x".repeat(65),
      ...Array.from({ length: 70 }, (_, index) => `opaque-${index}`),
    ]);

    expect(result.ids).toHaveLength(64);
    expect(result.ids[0]).toBe(ITEM_ID);
    expect(result.ids).not.toContain("bad id\nwith text");
    expect(result.ids).not.toContain("x".repeat(65));
    expect(result.invalidCount).toBe(1);
    expect(result.truncatedCount).toBe(8);
  });

  test("uses finite enums and bounded values for diagnostics", () => {
    expect(normalizeAriaChecked("unexpected-value")).toBe("unknown");
    expect(normalizeAriaChecked(null)).toBe("missing");

    const diagnostic = historyFilterDiagnostic(
      [ITEM_ID],
      [ITEM_ID],
      Number.POSITIVE_INFINITY,
      {
        allKindsAriaChecked: normalizeAriaChecked("unexpected-value"),
        linksAriaChecked: normalizeAriaChecked(undefined),
        menuPresent: true,
        menuExpanded: false,
      },
      listSnapshotWith({
        totalSize: Number.NaN,
        scrollTop: Number.POSITIVE_INFINITY,
        frame: 1818,
        rows: Array.from({ length: 80 }, () => ({
          id: "opaque-row",
          start: 0,
          height: 1,
          active: false,
          text: "must not be emitted",
        })),
      }),
    );

    expect(diagnostic.defaultTriggerCount).toBeNull();
    expect(diagnostic.menu).toEqual({
      allKindsAriaChecked: "unknown",
      linksAriaChecked: "missing",
      menuPresent: true,
      menuExpanded: false,
    });
    expect(diagnostic.list.totalSize).toBeNull();
    expect(diagnostic.list.scrollTop).toBeNull();
    expect(diagnostic.list.frame).toBe(1818);
    expect(diagnostic.list.visibleItemCount).toBeNull();
    expect(safeJSON(diagnostic)).not.toContain("must not be emitted");
  });

  test("caps a disjoint symmetric difference without truncating its JSON", () => {
    const before = Array.from({ length: 64 }, (_, index) => `before-${index}`);
    const restored = Array.from(
      { length: 64 },
      (_, index) => `restored-${index}`,
    );
    const diagnostic = historyFilterDiagnostic(
      before,
      restored,
      1,
      {
        allKindsAriaChecked: "false",
        linksAriaChecked: "true",
        menuPresent: true,
        menuExpanded: true,
      },
      listSnapshotWith(),
    );

    expect(diagnostic.matched).toBe(false);
    expect(diagnostic.symmetricDifference.ids).toHaveLength(64);
    expect(diagnostic.symmetricDifference.truncatedCount).toBe(64);
    const encoded = safeJSON(diagnostic);
    expect(encoded).not.toBe('{"diagnostic":"unavailable"}');
    expect(encoded.length).toBeLessThanOrEqual(16_384);
  });

  test("does not let a hostile serializer replace the timeout diagnostic", () => {
    const circular: { self?: unknown } = {};
    circular.self = circular;
    expect(safeJSON(circular)).toBe('{"diagnostic":"unavailable"}');
  });

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
