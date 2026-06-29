/**
 * Phase 4: §2 STYLEGUIDE dark-first theme default.
 *
 * Verifies that:
 *  1. DEFAULT_PREFS.theme is "dark" on first run (no localStorage).
 *  2. A user who had v1 prefs with theme:"dark" keeps it after migration.
 *  3. A user who had theme:"light" in v1 keeps "light" after migration.
 *  4. A user who had theme:"system" gets "dark" (system value removed in §2).
 *  5. setPrefs({ theme }) persists correctly.
 */

const PREFS_V1_KEY = "copypaste-ui-prefs-v1";
const PREFS_V3_KEY = "copypaste-ui-prefs-v3";
const PREFS_V4_KEY = "copypaste-ui-prefs-v4";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.resetModules();
});

async function freshStore() {
  const mod = await import("./store");
  return mod;
}

// ---------------------------------------------------------------------------
// 1. First-run default
// ---------------------------------------------------------------------------

test("first run (empty localStorage) defaults theme to 'dark'", async () => {
  const { useUI } = await freshStore();
  const prefs = useUI.getState().prefs;
  expect(prefs.theme).toBe("dark");
});

// ---------------------------------------------------------------------------
// 2. v1 migration: theme:'dark' is preserved
// ---------------------------------------------------------------------------

test("v1 migration: persisted theme:'dark' is preserved after migration", async () => {
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "dark" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("dark");
});

// ---------------------------------------------------------------------------
// 3. v1 migration: theme:'light' is preserved
// ---------------------------------------------------------------------------

test("v1 migration: explicit theme:'light' is preserved after migration", async () => {
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "light" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 4. v1 migration: theme:'system' is mapped to 'dark' (§2 STYLEGUIDE)
// ---------------------------------------------------------------------------

test("v1 migration: theme:'system' is mapped to 'dark' (system value removed in §2)", async () => {
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "system" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("dark");
});

// ---------------------------------------------------------------------------
// 5. v4 prefs with saved theme are honoured
// ---------------------------------------------------------------------------

test("v4 prefs with saved theme:'light' are respected", async () => {
  localStorage.setItem(PREFS_V4_KEY, JSON.stringify({ theme: "light" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 6. setPrefs can update theme and it persists
// ---------------------------------------------------------------------------

test("setPrefs({ theme: 'light' }) updates and persists the theme", async () => {
  const { useUI } = await freshStore();
  // Starts at default "dark"
  expect(useUI.getState().prefs.theme).toBe("dark");

  // User switches to light
  useUI.getState().setPrefs({ theme: "light" });
  expect(useUI.getState().prefs.theme).toBe("light");

  // Fresh store re-read from localStorage
  vi.resetModules();
  const { useUI: useUI2 } = await import("./store");
  expect(useUI2.getState().prefs.theme).toBe("light");
});
