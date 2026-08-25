import { describe, expect, it, vi } from "vitest";
import fuzzysort from "fuzzysort";

import { item } from "@/test/harness";
import {
  DEFAULT_VIEW,
  applyView,
  fuzzyItems,
  fuzzyTargets,
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
    const urls = applyView(rows, { ...DEFAULT_VIEW, kinds: ["url"] });
    expect(urls.map((r) => r.id)).toEqual(["a", "c"]);
  });

  it("does not reorder within a kind filter", () => {
    const urls = applyView(rows, { ...DEFAULT_VIEW, kinds: ["url"] });
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
      { ...DEFAULT_VIEW, kinds: ["secret"] },
    );
    expect(secrets.map((r) => r.id)).toEqual(["s"]);
  });

  it("returns an empty list rather than everything when nothing matches", () => {
    expect(applyView(rows, { ...DEFAULT_VIEW, kinds: ["color"] })).toHaveLength(0);
  });

  it("filters by stable device id rather than cosmetic name", () => {
    const devices = [
      fromDevice("device-b", "Phone", { id: "phone" }),
      fromDevice("device-a", "Phone", { id: "mac" }),
    ];
    const filtered = applyView(devices, {
      ...DEFAULT_VIEW,
      devices: ["device-a"],
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

/**
 * F-UI-2. The cache exists because `fuzzysort.single` re-indexes a raw string
 * on every call — 12.65 ms per keystroke at 200 × 2 KB, 0.05 ms prepared. It
 * holds plaintext, so what is asserted here is not only that it is used but
 * that a sensitive row can never enter it and that its contents are droppable.
 */
describe("prepared fuzzy targets", () => {
  it("returns the same result as the un-prepared filter", () => {
    const rows = [
      item({ id: "recent", content: "clipboard ocean" }),
      item({ id: "older", content: "clipboard orbit" }),
      item({ id: "exact", content: "clipboard" }),
    ];
    const targets = fuzzyTargets();
    expect(fuzzyItems(rows, "clipboard", targets).map((entry) => entry.id)).toEqual(
      fuzzyItems(rows, "clipboard").map((entry) => entry.id),
    );
  });

  it("prepares an item once and re-uses it across keystrokes", () => {
    const prepare = vi.spyOn(fuzzysort, "prepare");
    const rows = [item({ id: "a", content: "clipboard ocean" })];
    const targets = fuzzyTargets();

    for (const needle of ["c", "cl", "cli", "clip"]) {
      fuzzyItems(rows, needle, targets);
    }
    expect(prepare).toHaveBeenCalledTimes(1);
    prepare.mockRestore();
  });

  it("re-prepares when the row behind an id is not the row it cached", () => {
    const prepare = vi.spyOn(fuzzysort, "prepare");
    const targets = fuzzyTargets();
    fuzzyItems([item({ id: "a", content: "first" })], "fi", targets);
    fuzzyItems([item({ id: "a", content: "second" })], "se", targets);
    expect(prepare).toHaveBeenCalledTimes(2);
    prepare.mockRestore();
  });

  it("never prepares a sensitive row — the check runs before the write", () => {
    const prepare = vi.spyOn(fuzzysort, "prepare");
    const secret = item({ id: "secret", is_sensitive: true });
    const targets = fuzzyTargets();

    expect(targets.prepare(secret)).toBeNull();
    expect(fuzzyItems([secret], "secret", targets)).toEqual([]);
    expect(prepare).not.toHaveBeenCalled();
    prepare.mockRestore();
  });

  /** A revealed secret still arrives with `content: null` (INV-10); its
   *  plaintext lives only in `useReveal`, which INV-11 expires. */
  it("cannot hold a revealed secret, because a revealed row still has no content", () => {
    const targets = fuzzyTargets();
    expect(targets.prepare(item({ id: "s", is_sensitive: true }))).toBeNull();
    expect(targets.prepare(item({ id: "e", content: null }))).toBeNull();
  });

  it("drops a row it no longer holds, and everything on release", () => {
    const prepare = vi.spyOn(fuzzysort, "prepare");
    const row = item({ id: "a", content: "clipboard" });
    const targets = fuzzyTargets();

    fuzzyItems([row], "clip", targets);
    targets.retain([]);
    fuzzyItems([row], "clip", targets);
    expect(prepare).toHaveBeenCalledTimes(2);

    targets.release();
    fuzzyItems([row], "clip", targets);
    expect(prepare).toHaveBeenCalledTimes(3);
    prepare.mockRestore();
  });
});
