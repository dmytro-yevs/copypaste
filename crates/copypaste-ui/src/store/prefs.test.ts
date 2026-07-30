/**
 * INV-21 / AT-50, AT-51, AT-52 — preferences default **per field**.
 *
 * An invalid `theme` must not discard a valid `accent`. A present-but-invalid
 * field warns; an absent one defaults silently. Unknown keys are dropped and
 * never re-persisted. Malformed JSON, a non-object payload and a throwing
 * `localStorage` all fall back to full defaults without throwing.
 */
import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_PREFS, STORAGE_KEY, parsePrefs, readPrefs } from "./prefs";

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe("per-field recovery (AT-50)", () => {
  it("keeps the valid fields and resets only the invalid ones", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    const prefs = parsePrefs({
      theme: "chartreuse",
      accent: "teal",
      translucency: 5,
      previewLines: 4,
    });

    expect(prefs.theme).toBe(DEFAULT_PREFS.theme);
    expect(prefs.accent).toBe("teal"); // preserved
    expect(prefs.translucency).toBe(DEFAULT_PREFS.translucency);
    expect(prefs.previewLines).toBe(4); // preserved
    // One warning per present-but-invalid field, and no more.
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("defaults an absent field silently", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const prefs = parsePrefs({ accent: "rose" });
    expect(prefs).toEqual({ ...DEFAULT_PREFS, accent: "rose" });
    expect(warn).not.toHaveBeenCalled();
  });

  it("rejects an out-of-range preview-line count", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(parsePrefs({ previewLines: 99 }).previewLines).toBe(
      DEFAULT_PREFS.previewLines,
    );
    expect(parsePrefs({ previewLines: 2.5 }).previewLines).toBe(
      DEFAULT_PREFS.previewLines,
    );
  });
});

describe("unknown keys are dropped (AT-52)", () => {
  it("loads cleanly and does not carry them forward", () => {
    const prefs = parsePrefs({
      accent: "blue",
      density: "compact",
      historyDisplayLimit: 1000,
      __proto__: { polluted: true },
    });
    expect(prefs).toEqual({ ...DEFAULT_PREFS, accent: "blue" });
    expect(Object.keys(prefs).sort()).toEqual(Object.keys(DEFAULT_PREFS).sort());
  });
});

describe("malformed storage (AT-51)", () => {
  it.each([
    ["malformed JSON", "{not json"],
    ["a non-object payload", '"a string"'],
    ["an array", "[1,2,3]"],
    ["null", "null"],
  ])("%s falls back to full defaults without throwing", (_label, stored) => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    window.localStorage.setItem(STORAGE_KEY, stored);
    expect(() => readPrefs()).not.toThrow();
    expect(readPrefs()).toEqual(DEFAULT_PREFS);
  });

  it("survives a localStorage that throws", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    expect(readPrefs()).toEqual(DEFAULT_PREFS);
  });
});

describe("reading what zustand persisted", () => {
  it("unwraps the { state, version } envelope (INV-22)", () => {
    // This is the pre-paint path: it has to understand the same bytes the
    // store writes, or the first frame is themed from defaults and then jumps.
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { theme: "light", accent: "amber" }, version: 0 }),
    );
    const prefs = readPrefs();
    expect(prefs.theme).toBe("light");
    expect(prefs.accent).toBe("amber");
  });

  it("also reads a bare object, so an older or hand-written blob still loads", () => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ theme: "dark" }));
    expect(readPrefs().theme).toBe("dark");
  });
});
