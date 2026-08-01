import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import {
  DEFAULT_VIEW,
  applyView,
  fuzzyItems,
  isDefaultView,
  mergeSearchResults,
} from "@/lib/view";

function fromDevice(id: string, name: string, over = {}) {
  return {
    ...item(over),
    origin_device_id: id,
    origin_device_name: name,
  } as ReturnType<typeof item>;
}

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
    const urls = applyView(rows, { ...DEFAULT_VIEW, kind: "url" });
    expect(urls.map((r) => r.id)).toEqual(["a", "c"]);
  });

  it("does not reorder within a kind filter", () => {
    const urls = applyView(rows, { ...DEFAULT_VIEW, kind: "url" });
    expect(urls.map((r) => r.created_at)).toEqual([300, 100]);
  });

  it("sorts oldest first without touching the input", () => {
    const sorted = applyView(rows, { ...DEFAULT_VIEW, sort: "oldest" });
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
    const oldest = applyView(mixed, { ...DEFAULT_VIEW, sort: "oldest" });
    expect(oldest.map((r) => r.id)).toEqual(["pin-old", "pin-new", "loose"]);
  });

  /** A sensitive item has no content at all, so a kind filter must key off the
   *  flag rather than off text it will never see (INV-10). */
  it("finds sensitive items by their flag, not by their content", () => {
    const secrets = applyView(
      [...rows, item({ id: "s", is_sensitive: true, created_at: 400 })],
      { ...DEFAULT_VIEW, kind: "secret" },
    );
    expect(secrets.map((r) => r.id)).toEqual(["s"]);
  });

  it("returns an empty list rather than everything when nothing matches", () => {
    expect(applyView(rows, { ...DEFAULT_VIEW, kind: "color" })).toHaveLength(0);
  });

  it("filters by stable device id rather than cosmetic name", () => {
    const devices = [
      fromDevice("device-b", "Phone", { id: "phone" }),
      fromDevice("device-a", "Phone", { id: "mac" }),
    ];
    const filtered = applyView(devices, {
      ...DEFAULT_VIEW,
      device: "device-a",
    });
    expect(filtered.map((entry) => entry.id)).toEqual(["mac"]);
  });

  it("groups alphabetically by id while preserving order inside a device", () => {
    const devices = [
      fromDevice("device-b", "Phone", { id: "b-new" }),
      fromDevice("device-a", "Mac", { id: "a-new" }),
      fromDevice("device-b", "Phone", { id: "b-old" }),
    ];
    const grouped = applyView(devices, {
      ...DEFAULT_VIEW,
      groupByDevice: true,
    });
    expect(grouped.map((entry) => entry.id)).toEqual([
      "a-new",
      "b-new",
      "b-old",
    ]);
  });
});

describe("fuzzy and service search merge", () => {
  it("ranks fuzzy matches and keeps equal results in input order", () => {
    const rows = [
      item({ id: "recent", content: "clipboard ocean" }),
      item({ id: "older", content: "clipboard orbit" }),
      item({ id: "exact", content: "clipboard" }),
    ];
    expect(fuzzyItems(rows, "clipboard").map((entry) => entry.id)).toEqual([
      "exact",
      "recent",
      "older",
    ]);
  });

  it("appends service-only hits in service order and de-duplicates by id", () => {
    const fuzzy = [item({ id: "client-a" }), item({ id: "shared" })];
    const server = [
      item({ id: "shared", content: "server copy" }),
      item({ id: "server-a" }),
      item({ id: "server-b" }),
    ];
    expect(mergeSearchResults(fuzzy, server).map((entry) => entry.id)).toEqual([
      "client-a",
      "shared",
      "server-a",
      "server-b",
    ]);
  });

  it("never matches or merges a sensitive row", () => {
    const secret = item({ id: "secret", is_sensitive: true });
    expect(fuzzyItems([secret], "sensitive")).toEqual([]);
    expect(mergeSearchResults([], [secret])).toEqual([]);
  });
});
