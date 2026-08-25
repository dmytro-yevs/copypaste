/**
 * INV-21 / AT-50, AT-51, AT-52 — preferences default **per field**.
 *
 * An invalid mode must not discard a valid product theme. A present-but-invalid
 * field warns; an absent one defaults silently. Unknown keys are dropped and
 * never re-persisted. Malformed JSON, a non-object payload and a throwing
 * `localStorage` all fall back to full defaults without throwing.
 */
import { act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const nativePreferences = vi.hoisted(() => ({
  get: vi.fn(),
  set: vi.fn(),
  save: vi.fn(),
  delete: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: vi.fn(function LazyStore() {
    return nativePreferences;
  }),
}));

import {
  DEFAULT_PREFS,
  PREFERENCES_VERSION,
  STORAGE_KEY,
  parsePrefs,
  readPrefs,
  usePrefs,
} from "./prefs";

afterEach(() => {
  vi.restoreAllMocks();
  nativePreferences.get.mockReset();
  nativePreferences.set.mockReset();
  nativePreferences.save.mockReset();
  nativePreferences.delete.mockReset();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  window.localStorage.clear();
  usePrefs.setState(DEFAULT_PREFS);
});

const currentEnvelope = (state: unknown) => ({
  state,
  version: PREFERENCES_VERSION,
});

describe("per-field recovery (AT-50)", () => {
  it("keeps the valid fields and resets only the invalid ones", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    const prefs = parsePrefs({
      theme: "chartreuse",
      colorTheme: "aurora",
      translucency: 101,
      previewLines: 3,
    });

    expect(prefs.theme).toBe(DEFAULT_PREFS.theme);
    expect(prefs.colorTheme).toBe("aurora");
    expect(prefs.translucency).toBe(DEFAULT_PREFS.translucency);
    expect(prefs.previewLines).toBe(3); // preserved
    // One warning per present-but-invalid field, and no more.
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("defaults an absent field silently", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const prefs = parsePrefs({ colorTheme: "ember" });
    expect(prefs).toEqual({
      ...DEFAULT_PREFS,
      colorTheme: "ember",
    });
    expect(warn).not.toHaveBeenCalled();
  });

  it("accepts only the current translucency switch", () => {
    expect(parsePrefs({ translucency: true }).translucency).toBe(true);
    expect(parsePrefs({ translucency: false }).translucency).toBe(false);
    expect(parsePrefs({ translucency: 45 }).translucency).toBe(
      DEFAULT_PREFS.translucency,
    );
  });

  it("rejects an out-of-range preview-line count", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(parsePrefs({ previewLines: 99 }).previewLines).toBe(
      DEFAULT_PREFS.previewLines,
    );
    expect(parsePrefs({ previewLines: 4 }).previewLines).toBe(
      DEFAULT_PREFS.previewLines,
    );
    expect(parsePrefs({ previewLines: 1 }).previewLines).toBe(
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
      colorTheme: "graphite",
      density: "compact",
      oldHistoryDisplayLimit: 1000,
      __proto__: { polluted: true },
    });
    expect(prefs).toEqual({
      ...DEFAULT_PREFS,
      colorTheme: "graphite",
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
  it("reads the current { state, version } envelope (INV-22)", () => {
    // This is the pre-paint path: it has to understand the same bytes the
    // store writes, or the first frame is themed from defaults and then jumps.
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(currentEnvelope({ theme: "light", colorTheme: "aurora" })),
    );
    const prefs = readPrefs();
    expect(prefs.theme).toBe("light");
    expect(prefs.colorTheme).toBe("aurora");
  });

  it("ignores an envelope from another preference version", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { theme: "dark" }, version: 1 }),
    );
    expect(readPrefs()).toEqual(DEFAULT_PREFS);
  });

  it("ignores a bare preference object", () => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ theme: "dark" }));
    expect(readPrefs()).toEqual(DEFAULT_PREFS);
  });

  it("does not let zustand migrate a version-1 preference", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { theme: "dark" }, version: 1 }),
    );

    await act(async () => usePrefs.persist.rehydrate());

    expect(usePrefs.getState().theme).toBe(DEFAULT_PREFS.theme);
  });
});

describe("native preference authority", () => {
  it("ignores browser storage when the native store has no value", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    nativePreferences.get.mockResolvedValue(undefined);
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(currentEnvelope({ theme: "dark" })),
    );

    await act(async () => usePrefs.persist.rehydrate());

    expect(nativePreferences.get).toHaveBeenCalledWith(STORAGE_KEY);
    expect(nativePreferences.set).not.toHaveBeenCalled();
    expect(usePrefs.getState().theme).toBe(DEFAULT_PREFS.theme);
  });

  it("uses defaults rather than browser storage after a native read failure", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    nativePreferences.get.mockRejectedValue(new Error("unavailable"));
    vi.spyOn(console, "warn").mockImplementation(() => {});
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(currentEnvelope({ theme: "dark" })),
    );

    await act(async () => usePrefs.persist.rehydrate());

    expect(usePrefs.getState().theme).toBe(DEFAULT_PREFS.theme);
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
      JSON.stringify(currentEnvelope({ allowScreenshots: true })),
    );
    expect(readPrefs().allowScreenshots).toBe(true);
  });
});

describe("first-run onboarding completion", () => {
  it("starts incomplete when nothing has been stored", () => {
    expect(DEFAULT_PREFS.onboardingComplete).toBe(false);
    expect(parsePrefs({}).onboardingComplete).toBe(false);
  });

  it("does not skip onboarding when a stored blob has no completion field", () => {
    expect(parsePrefs({ theme: "dark" }).onboardingComplete).toBe(false);
  });

  it("honours an explicit stored flag", () => {
    expect(parsePrefs({ onboardingComplete: true }).onboardingComplete).toBe(true);
    expect(parsePrefs({ onboardingComplete: false }).onboardingComplete).toBe(
      false,
    );
  });
});
