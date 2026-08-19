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
      translucency: 101,
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
    expect(prefs).toEqual({
      ...DEFAULT_PREFS,
      accent: "rose",
      onboardingComplete: true,
    });
    expect(warn).not.toHaveBeenCalled();
  });

  it("persists the system accent choice", () => {
    expect(parsePrefs({ accent: "system" }).accent).toBe("system");
  });

  it("accepts a bounded translucency percentage and upgrades the alpha switch", () => {
    expect(parsePrefs({ translucency: 45 }).translucency).toBe(45);
    expect(parsePrefs({ translucency: true }).translucency).toBe(100);
    expect(parsePrefs({ translucency: false }).translucency).toBe(0);
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

  it("bounds every display preference independently", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const prefs = parsePrefs({
      previewLinesPopup: 0,
      historyDisplayLimit: 999,
      sortByDevice: true,
    });
    expect(prefs.previewLinesPopup).toBe(DEFAULT_PREFS.previewLinesPopup);
    expect(prefs.historyDisplayLimit).toBe(DEFAULT_PREFS.historyDisplayLimit);
    expect(prefs.sortByDevice).toBe(true);
  });

  it("accepts the unlimited display sentinel", () => {
    expect(parsePrefs({ historyDisplayLimit: 100_000 }).historyDisplayLimit).toBe(
      100_000,
    );
  });
});

describe("unknown keys are dropped (AT-52)", () => {
  it("loads cleanly and does not carry them forward", () => {
    const prefs = parsePrefs({
      accent: "blue",
      density: "compact",
      oldHistoryDisplayLimit: 1000,
      __proto__: { polluted: true },
    });
    expect(prefs).toEqual({
      ...DEFAULT_PREFS,
      accent: "blue",
      onboardingComplete: true,
    });
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

/**
 * INV-35. The window is created content-protected and the only thing that
 * relaxes it is this preference, so every way it can fail to load has to land
 * on "not allowed".
 */
describe("the screenshot preference fails towards protected (INV-35)", () => {
  it("is off by default", () => {
    expect(DEFAULT_PREFS.allowScreenshots).toBe(false);
  });

  it("is off when the stored value is not a boolean", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(parsePrefs({ allowScreenshots: "yes" }).allowScreenshots).toBe(false);
    expect(parsePrefs({ allowScreenshots: 1 }).allowScreenshots).toBe(false);
  });

  it("is off when storage could not be read at all", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    expect(readPrefs().allowScreenshots).toBe(false);
  });

  it("is on only when the user actually stored that", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { allowScreenshots: true }, version: 0 }),
    );
    expect(readPrefs().allowScreenshots).toBe(true);
  });
});

describe("first-run onboarding completion", () => {
  it("starts incomplete when nothing has been stored", () => {
    expect(DEFAULT_PREFS.onboardingComplete).toBe(false);
    expect(parsePrefs({}).onboardingComplete).toBe(false);
  });

  it("treats an existing preferences blob as already past first-run", () => {
    expect(parsePrefs({ theme: "dark" }).onboardingComplete).toBe(true);
  });

  it("honours an explicit stored flag", () => {
    expect(parsePrefs({ onboardingComplete: true }).onboardingComplete).toBe(true);
    expect(parsePrefs({ onboardingComplete: false }).onboardingComplete).toBe(
      false,
    );
  });
});
