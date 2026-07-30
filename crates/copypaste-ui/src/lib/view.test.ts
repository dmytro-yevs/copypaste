import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import { DEFAULT_VIEW, applyView, isDefaultView } from "@/lib/view";

describe("applyView", () => {
  const rows = [
    item({ id: "a", content: "https://example.com", created_at: 300 }),
    item({ id: "b", content: "plain words", created_at: 200 }),
    item({ id: "c", content: "https://other.test", created_at: 100 }),
  ];

  /** INV-2, at the one place a filter would quietly break it. */
  it("returns the identical array when the view is the default", () => {
    expect(applyView(rows, DEFAULT_VIEW)).toBe(rows);
    expect(isDefaultView(DEFAULT_VIEW)).toBe(true);
  });

  it("keeps only the chosen kind", () => {
    const urls = applyView(rows, { kind: "url", sort: "newest" });
    expect(urls.map((r) => r.id)).toEqual(["a", "c"]);
  });

  it("does not reorder within a kind filter", () => {
    const urls = applyView(rows, { kind: "url", sort: "newest" });
    expect(urls.map((r) => r.created_at)).toEqual([300, 100]);
  });

  it("sorts oldest first without touching the input", () => {
    const sorted = applyView(rows, { kind: "all", sort: "oldest" });
    expect(sorted.map((r) => r.id)).toEqual(["c", "b", "a"]);
    expect(rows.map((r) => r.id)).toEqual(["a", "b", "c"]);
  });

  /**
   * Pinning is a section, not a date. A user pins something so it stops
   * moving; an "oldest first" that buries their pins at the bottom is the one
   * outcome pinning exists to prevent.
   */
  it("keeps pinned items ahead of unpinned ones in both orders", () => {
    const mixed = [
      item({ id: "pin-new", pinned: true, created_at: 500 }),
      item({ id: "pin-old", pinned: true, created_at: 100 }),
      item({ id: "loose", pinned: false, created_at: 900 }),
    ];
    const oldest = applyView(mixed, { kind: "all", sort: "oldest" });
    expect(oldest.map((r) => r.id)).toEqual(["pin-old", "pin-new", "loose"]);
  });

  /** A sensitive item has no content at all, so a kind filter must key off the
   *  flag rather than off text it will never see (INV-10). */
  it("finds sensitive items by their flag, not by their content", () => {
    const secrets = applyView(
      [...rows, item({ id: "s", is_sensitive: true, created_at: 400 })],
      { kind: "secret", sort: "newest" },
    );
    expect(secrets.map((r) => r.id)).toEqual(["s"]);
  });

  it("returns an empty list rather than everything when nothing matches", () => {
    expect(applyView(rows, { kind: "color", sort: "newest" })).toHaveLength(0);
  });
});
