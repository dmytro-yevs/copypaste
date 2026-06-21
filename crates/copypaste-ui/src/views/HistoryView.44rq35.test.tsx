/**
 * CopyPaste-44rq.35 — itemsSignature memoisation
 *
 * The poll loop (every 3 s, up to 200 items) called itemsSignature on every
 * tick even when nothing changed.  The fix adds a 1-slot cache keyed on
 * (length, first fingerprint, last fingerprint) so the O(n) map+join is
 * skipped when the clipboard is idle.
 *
 * These tests verify:
 *   1. Stable return: identical inputs produce the same string.
 *   2. Cache hit: the _itemsSigCache entry is reused (no recomputation) when
 *      called with same-content arrays.
 *   3. Cache miss: new items (changed length) invalidate the cache.
 *   4. Cache miss: a pin toggle on the first item invalidates the cache.
 *   5. Empty array returns "".
 */
import { describe, it, expect, beforeEach } from "vitest";

// We import directly from the module — Tauri is NOT needed for these unit tests.
// The module-level _itemsSigCache is reset between tests so tests are isolated.
import { itemsSignature, _itemsSigCache } from "./HistoryView";

// Re-export hack: since _itemsSigCache is a `let` binding we need to read it
// from the live module binding after each call, so we use a dynamic import
// accessor.  Vitest handles ESM live bindings correctly.
// We import the full namespace so we can observe the cache after each call.
import * as HV from "./HistoryView";

// ---------------------------------------------------------------------------
// Tauri mocks (module-level; needed because HistoryView.tsx imports Tauri at
// module scope even though these tests never render the component).
// ---------------------------------------------------------------------------
import { vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: vi.fn() }));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type Entry = Parameters<typeof itemsSignature>[0][number];

function makeEntry(id: string, wallTime = 1_700_000_000_000, pinned = false): Entry {
  return {
    id,
    content_type: "text" as const,
    preview: `Item ${id}`,
    is_sensitive: false,
    wall_time: wallTime,
    pinned,
    origin_device_id: "",
  };
}

// Reset the module-level cache between tests so they don't bleed into each other.
beforeEach(() => {
  // The cache is a module-level `let` — reassign via the live binding trick:
  // itemsSignature() itself will overwrite it; we force a miss by calling with
  // a dummy empty array once, or we can just call with a different length.
  // Simplest: call the function with an empty array to ensure next non-empty
  // call does a full recompute.
  itemsSignature([]);
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("itemsSignature — memoisation (CopyPaste-44rq.35)", () => {
  it("returns identical string for the same items called twice", () => {
    const items = [makeEntry("a"), makeEntry("b"), makeEntry("c")];
    const sig1 = itemsSignature(items);
    const sig2 = itemsSignature(items);
    expect(sig1).toBe(sig2);
  });

  it("returns identical string for two different array instances with the same content", () => {
    const items1 = [makeEntry("a", 100), makeEntry("b", 200)];
    const items2 = [makeEntry("a", 100), makeEntry("b", 200)];
    expect(itemsSignature(items1)).toBe(itemsSignature(items2));
  });

  it("hits the cache on second call with same content (no full recomputation)", () => {
    const items = [makeEntry("x", 1), makeEntry("y", 2), makeEntry("z", 3)];

    // First call: populates the cache.
    const sig1 = itemsSignature(items);
    expect(HV._itemsSigCache).not.toBeNull();
    expect(HV._itemsSigCache?.result).toBe(sig1);

    // Capture cache reference — it must be the SAME object after second call
    // (no new object allocated means we took the fast path).
    const cacheBefore = HV._itemsSigCache;

    // Second call with different array reference but same content.
    const items2 = [makeEntry("x", 1), makeEntry("y", 2), makeEntry("z", 3)];
    const sig2 = itemsSignature(items2);

    expect(sig2).toBe(sig1);
    // Cache object identity is preserved: the fast path returned early without
    // allocating a new cache entry.
    expect(HV._itemsSigCache).toBe(cacheBefore);
  });

  it("invalidates cache when a new item is appended (length changes)", () => {
    const base = [makeEntry("a", 1), makeEntry("b", 2)];
    const sig1 = itemsSignature(base);
    const cacheBefore = HV._itemsSigCache;

    const extended = [makeEntry("a", 1), makeEntry("b", 2), makeEntry("c", 3)];
    const sig2 = itemsSignature(extended);

    expect(sig2).not.toBe(sig1);
    // A new cache entry was created.
    expect(HV._itemsSigCache).not.toBe(cacheBefore);
    expect(HV._itemsSigCache?.result).toBe(sig2);
  });

  it("invalidates cache when the first item is pinned (fingerprint changes)", () => {
    const items = [makeEntry("a", 1, false), makeEntry("b", 2)];
    const sig1 = itemsSignature(items);

    // Pin the first item — daemon would also reorder but here we test the
    // signature function directly: the `pinned` flag changes the fingerprint.
    const pinned = [makeEntry("a", 1, true), makeEntry("b", 2)];
    const sig2 = itemsSignature(pinned);

    expect(sig2).not.toBe(sig1);
  });

  it("invalidates cache when the last item changes wall_time", () => {
    const items = [makeEntry("a", 1), makeEntry("b", 2)];
    const sig1 = itemsSignature(items);

    const updated = [makeEntry("a", 1), makeEntry("b", 999)];
    const sig2 = itemsSignature(updated);

    expect(sig2).not.toBe(sig1);
  });

  it("returns empty string for an empty array", () => {
    expect(itemsSignature([])).toBe("");
  });

  it("single item: includes id, pinned flag, and wall_time", () => {
    const sig = itemsSignature([makeEntry("abc", 123, true)]);
    expect(sig).toBe("abc:1:123");
  });

  it("multiple items are pipe-separated", () => {
    const sig = itemsSignature([makeEntry("a", 1), makeEntry("b", 2)]);
    expect(sig).toBe("a:0:1|b:0:2");
  });
});
