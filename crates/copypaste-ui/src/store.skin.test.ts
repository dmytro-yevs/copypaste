/**
 * Tests for the `skin` pref added in W-F2.
 * Mirrors the palette pref plumbing — default, getter/setter, v2→v3 migration.
 */

// jsdom localStorage reset between tests
beforeEach(() => {
  localStorage.clear();
});

// Vitest's module cache must be reset so loadPrefs() re-reads localStorage
// each time we manipulate it before importing the module.
afterEach(() => {
  vi.resetModules();
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function freshStore() {
  // Dynamic import so vi.resetModules() above takes effect.
  const mod = await import("./store");
  return mod;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("UIPrefs.skin — W-F2 plumbing", () => {
  it("default skin is 'classic'", async () => {
    const { useUI } = await freshStore();
    const prefs = useUI.getState().prefs;
    expect(prefs.skin).toBe("classic");
  });

  it("setPrefs updates skin and persists to localStorage under v3 key", async () => {
    const { useUI } = await freshStore();
    useUI.getState().setPrefs({ skin: "quiet" });
    const prefs = useUI.getState().prefs;
    expect(prefs.skin).toBe("quiet");

    // Verify it was persisted under the v3 key
    const raw = localStorage.getItem("copypaste-ui-prefs-v3");
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.skin).toBe("quiet");
  });

  it("v2→v3 migration: existing v2 prefs get skin:'classic' injected", async () => {
    // Write a v2-style prefs blob (no 'skin' field) under the v2 key.
    const v2Prefs = {
      previewLinesApp: 2,
      previewLinesPopup: 3,
      previewSize: 32,
      maskSensitive: false,
      imageMaxHeight: 60,
      playSoundOnCopy: false,
      notifyOnCopy: false,
      translucency: false,
      theme: "dark",
      density: "spacious",
      palette: "ocean-blue",
      motionReduced: true,
      historyDisplayLimit: 500,
      showSensitiveWarnings: false,
    };
    localStorage.setItem("copypaste-ui-prefs-v2", JSON.stringify(v2Prefs));

    const { useUI } = await freshStore();
    const prefs = useUI.getState().prefs;

    // skin must be injected with default 'classic'
    expect(prefs.skin).toBe("classic");

    // All other v2 prefs must be preserved verbatim
    expect(prefs.previewLinesApp).toBe(2);
    expect(prefs.previewLinesPopup).toBe(3);
    expect(prefs.theme).toBe("dark");
    expect(prefs.palette).toBe("ocean-blue");
    expect(prefs.density).toBe("spacious");
    expect(prefs.motionReduced).toBe(true);

    // Re-persisted under v3 key
    const raw = localStorage.getItem("copypaste-ui-prefs-v3");
    expect(raw).not.toBeNull();
    const parsed = JSON.parse(raw!);
    expect(parsed.skin).toBe("classic");
  });

  it("v2→v3 migration: v2 key is removed after migration", async () => {
    const v2Prefs = { palette: "graphite-mist", theme: "light" };
    localStorage.setItem("copypaste-ui-prefs-v2", JSON.stringify(v2Prefs));

    await freshStore();

    expect(localStorage.getItem("copypaste-ui-prefs-v2")).toBeNull();
  });

  it("existing v3 prefs with skin are loaded without alteration", async () => {
    const v3Prefs = {
      previewLinesApp: 1,
      previewLinesPopup: 1,
      previewSize: 28,
      maskSensitive: true,
      imageMaxHeight: 40,
      playSoundOnCopy: true,
      notifyOnCopy: true,
      translucency: true,
      theme: "light",
      density: "compact",
      palette: "graphite-mist",
      motionReduced: false,
      historyDisplayLimit: 1000,
      showSensitiveWarnings: true,
      skin: "vapor",
    };
    localStorage.setItem("copypaste-ui-prefs-v3", JSON.stringify(v3Prefs));

    const { useUI } = await freshStore();
    expect(useUI.getState().prefs.skin).toBe("vapor");
  });

  it("setPrefs({skin}) does not clobber other prefs", async () => {
    const { useUI } = await freshStore();
    useUI.getState().setPrefs({ palette: "ocean-blue" });
    useUI.getState().setPrefs({ skin: "vapor" });

    const prefs = useUI.getState().prefs;
    expect(prefs.palette).toBe("ocean-blue");
    expect(prefs.skin).toBe("vapor");
    // Verify translucency + motionReduced still intact
    expect(prefs.translucency).toBe(true);
    expect(prefs.motionReduced).toBe(false);
  });
});
