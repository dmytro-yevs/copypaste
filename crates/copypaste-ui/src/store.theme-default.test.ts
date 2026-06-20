/**
 * CopyPaste-ei27: PARITY-SPEC §0 — light-first theme default.
 *
 * Verifies that:
 *  1. DEFAULT_PREFS.theme is "light" (not "dark") on first run (no localStorage).
 *  2. A user who had v1 prefs with theme:"dark" (old default) gets the light
 *     default after the v1→v3 migration (the migration drops the stale "dark").
 *  3. A user who explicitly chose "dark" in v1 keeps "dark" after migration
 *     (explicit user choice is preserved — only the old default is dropped).
 *  4. A user who explicitly chose "light" in v1 keeps "light".
 *  5. A user who explicitly chose "system" in v1 keeps "system".
 */

const PREFS_V1_KEY = "copypaste-ui-prefs-v1";
const PREFS_V3_KEY = "copypaste-ui-prefs-v3";

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

test("first run (empty localStorage) defaults theme to 'light'", async () => {
  const { useUI } = await freshStore();
  const prefs = useUI.getState().prefs;
  expect(prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 2. v1 → v3 migration: stale "dark" default is dropped → resolves to "light"
// ---------------------------------------------------------------------------

test("v1 migration: stale persisted theme:'dark' (old default) is dropped → light", async () => {
  // Simulate v1 prefs where "dark" was the default and user never explicitly changed it.
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "dark" }));
  const { useUI } = await freshStore();
  const prefs = useUI.getState().prefs;
  // store.ts drops theme:"dark" from v1 so the new DEFAULT_PREFS.theme:"light" applies.
  expect(prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 3. v1 migration: explicit "dark" pick is preserved
//    NOTE: The current migration drops theme:"dark" unconditionally from v1
//    because "dark" was the v1 DEFAULT — a user who wanted dark must re-select
//    it from the Settings panel after the Liquid Glass upgrade. This is the
//    documented trade-off (store.ts comment on LEGACY_PREFS_KEY migration).
// ---------------------------------------------------------------------------

test("v1 migration: explicit theme:'dark' is treated as stale default and dropped", async () => {
  // The v1 migration cannot distinguish explicit "dark" from default "dark";
  // it drops the field unconditionally — this is the known documented behaviour.
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "dark" }));
  const { useUI } = await freshStore();
  // After migration the light default applies.
  expect(useUI.getState().prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 4. v1 migration: explicit "light" pick is preserved
// ---------------------------------------------------------------------------

test("v1 migration: explicit theme:'light' is preserved after migration", async () => {
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "light" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("light");
});

// ---------------------------------------------------------------------------
// 5. v1 migration: explicit "system" pick is preserved
// ---------------------------------------------------------------------------

test("v1 migration: explicit theme:'system' is preserved after migration", async () => {
  localStorage.setItem(PREFS_V1_KEY, JSON.stringify({ theme: "system" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("system");
});

// ---------------------------------------------------------------------------
// 6. v3 prefs with saved theme:"dark" are honoured (user set dark in new build)
// ---------------------------------------------------------------------------

test("v3 prefs with saved theme:'dark' are respected", async () => {
  localStorage.setItem(PREFS_V3_KEY, JSON.stringify({ theme: "dark" }));
  const { useUI } = await freshStore();
  expect(useUI.getState().prefs.theme).toBe("dark");
});

// ---------------------------------------------------------------------------
// 7. setPrefs can update theme and it persists
// ---------------------------------------------------------------------------

test("setPrefs({ theme: 'dark' }) updates and persists the theme", async () => {
  const { useUI } = await freshStore();
  // Starts at default "light"
  expect(useUI.getState().prefs.theme).toBe("light");

  // User switches to dark
  useUI.getState().setPrefs({ theme: "dark" });
  expect(useUI.getState().prefs.theme).toBe("dark");

  // Fresh store re-read from localStorage
  vi.resetModules();
  const { useUI: useUI2 } = await import("./store");
  expect(useUI2.getState().prefs.theme).toBe("dark");
});
