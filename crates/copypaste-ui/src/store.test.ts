import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFS, loadPrefs, useUI } from "./store";
import { PREFS_KEY } from "./lib/theme/prefsSchema";

// loadPrefs() is a pure function of localStorage. These tests seed the key and
// assert the whitelist-merge + per-field validation contract (design.md
// Decision 10). There is NO migration path to test — legacy v1/v2/v3 keys were
// removed — so no migration tests exist by design.

function seed(value: unknown): void {
  localStorage.setItem(PREFS_KEY, JSON.stringify(value));
}

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(console, "warn").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("loadPrefs — appearance-field validation & merge", () => {
  it("returns full defaults when nothing is stored", () => {
    expect(loadPrefs()).toEqual(DEFAULT_PREFS);
  });

  it("malformed JSON → full DEFAULT_PREFS (logged, not thrown)", () => {
    localStorage.setItem(PREFS_KEY, "{not valid json");
    expect(loadPrefs()).toEqual(DEFAULT_PREFS);
    expect(console.warn).toHaveBeenCalled();
  });

  it("a non-object payload (array/number/null) → full defaults", () => {
    seed([1, 2, 3]);
    expect(loadPrefs()).toEqual(DEFAULT_PREFS);
    seed(42);
    expect(loadPrefs()).toEqual(DEFAULT_PREFS);
    localStorage.setItem(PREFS_KEY, "null");
    expect(loadPrefs()).toEqual(DEFAULT_PREFS);
  });

  it("unknown keys are dropped and never resurface", () => {
    seed({ theme: "light", bogusKey: "x", anotherJunk: 5 });
    const prefs = loadPrefs() as Record<string, unknown>;
    expect(prefs.bogusKey).toBeUndefined();
    expect(prefs.anotherJunk).toBeUndefined();
    expect(prefs.theme).toBe("light");
  });

  it("a blob predating the appearance fields gains them at defaults", () => {
    // Only legacy (still-known) fields present; the three appearance axes absent.
    seed({ previewLinesApp: 3, historyDisplayLimit: 500 });
    const prefs = loadPrefs();
    expect(prefs.previewLinesApp).toBe(3);
    expect(prefs.historyDisplayLimit).toBe(500);
    expect(prefs.theme).toBe(DEFAULT_PREFS.theme);
    expect(prefs.accent).toBe(DEFAULT_PREFS.accent);
    expect(prefs.translucency).toBe(DEFAULT_PREFS.translucency);
  });

  it("an invalid theme defaults while a valid accent is kept", () => {
    seed({ theme: "neon", accent: "teal" });
    const prefs = loadPrefs();
    expect(prefs.theme).toBe("dark"); // defaulted
    expect(prefs.accent).toBe("teal"); // preserved
    expect(console.warn).toHaveBeenCalled();
  });

  it("theme:\"system\" is a valid ThemeValue and round-trips (CopyPaste-g27b.20)", () => {
    seed({ theme: "system", accent: "teal" });
    const prefs = loadPrefs();
    expect(prefs.theme).toBe("system"); // not defaulted
    expect(prefs.accent).toBe("teal");
    expect(console.warn).not.toHaveBeenCalled();
  });

  it("an invalid accent defaults while a valid theme is kept", () => {
    seed({ theme: "light", accent: "chartreuse" });
    const prefs = loadPrefs();
    expect(prefs.theme).toBe("light");
    expect(prefs.accent).toBe("indigo");
  });

  it("a non-boolean translucency defaults to true, keeping other fields", () => {
    seed({ theme: "light", accent: "rose", translucency: "yes" });
    const prefs = loadPrefs();
    expect(prefs.theme).toBe("light");
    expect(prefs.accent).toBe("rose");
    expect(prefs.translucency).toBe(true);
  });

  it("each of the 6 accents and both themes round-trip", () => {
    for (const accent of ["indigo", "blue", "teal", "green", "amber", "rose"]) {
      for (const theme of ["dark", "light"]) {
        seed({ theme, accent, translucency: false });
        const prefs = loadPrefs();
        expect(prefs.theme).toBe(theme);
        expect(prefs.accent).toBe(accent);
        expect(prefs.translucency).toBe(false);
      }
    }
  });

  it("a full valid blob round-trips unchanged through save→load", () => {
    const stored = { ...DEFAULT_PREFS, theme: "light", accent: "amber", translucency: false };
    seed(stored);
    expect(loadPrefs()).toEqual(stored);
  });

  it("reloadPrefs re-reads persisted prefs into the live store (warm popup WebView)", () => {
    // Simulate a Settings change persisted after the popup's one-time loadPrefs().
    seed({ ...DEFAULT_PREFS, theme: "light", accent: "amber", translucency: false });
    useUI.getState().reloadPrefs();
    const p = useUI.getState().prefs;
    expect(p.theme).toBe("light");
    expect(p.accent).toBe("amber");
    expect(p.translucency).toBe(false);
    // And an invalid stored value still defaults per-field on reload.
    seed({ ...DEFAULT_PREFS, theme: "neon", accent: "teal" });
    useUI.getState().reloadPrefs();
    expect(useUI.getState().prefs.theme).toBe("dark");
    expect(useUI.getState().prefs.accent).toBe("teal");
    // theme:"system" is valid (CopyPaste-g27b.20) and round-trips on reload too.
    seed({ ...DEFAULT_PREFS, theme: "system", accent: "rose" });
    useUI.getState().reloadPrefs();
    expect(useUI.getState().prefs.theme).toBe("system");
    expect(useUI.getState().prefs.accent).toBe("rose");
  });

  it("a localStorage access exception falls back to defaults", () => {
    const spy = vi.spyOn(Storage.prototype, "getItem");
    // The polyfilled localStorage isn't a Storage instance; patch the instance.
    const orig = localStorage.getItem;
    localStorage.getItem = () => {
      throw new Error("SecurityError: storage disabled");
    };
    try {
      expect(loadPrefs()).toEqual(DEFAULT_PREFS);
      expect(console.warn).toHaveBeenCalled();
    } finally {
      localStorage.getItem = orig;
      spy.mockRestore();
    }
  });
});
