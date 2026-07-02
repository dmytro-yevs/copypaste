/**
 * historyBadge.test.ts — toolbar count badge (CopyPaste-g27b.37).
 *
 * The badge previously always showed `totalCount` (the full daemon-side DB
 * count), even while a search query or device filter narrowed the visible
 * list to zero matches — e.g. "14 items" next to an empty "No matches"
 * result. historyBadgeCount() picks the FILTERED count whenever a search or
 * device filter is active, and the unfiltered daemon total otherwise.
 */
import { describe, expect, it } from "vitest";
import { historyBadgeCount } from "./historyBadge";

describe("historyBadgeCount", () => {
  it("returns null (badge hidden) while totalCount hasn't resolved yet", () => {
    expect(
      historyBadgeCount({ totalCount: null, filteredCount: 0, search: "", deviceFilter: "all" }),
    ).toBeNull();
  });

  it("no search, no device filter: shows the unfiltered daemon total", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 14, search: "", deviceFilter: "all" }),
    ).toBe(14);
  });

  it("active search with zero matches: shows 0, not the stale total", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 0, search: "zzzzz-no-match", deviceFilter: "all" }),
    ).toBe(0);
  });

  it("active search with some matches: shows the filtered count", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 3, search: "report", deviceFilter: "all" }),
    ).toBe(3);
  });

  it("whitespace-only search is treated as no search (matches useHistoryFilter's search.trim() contract)", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 0, search: "   ", deviceFilter: "all" }),
    ).toBe(14);
  });

  it("device filter active (no search): shows the filtered count", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 5, search: "", deviceFilter: "device-abc" }),
    ).toBe(5);
  });

  it("both search and device filter active: still shows the filtered count", () => {
    expect(
      historyBadgeCount({ totalCount: 14, filteredCount: 1, search: "x", deviceFilter: "device-abc" }),
    ).toBe(1);
  });
});
