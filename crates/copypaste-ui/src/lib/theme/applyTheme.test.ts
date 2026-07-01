import { afterEach, describe, expect, it, vi } from "vitest";
import { applyAppearanceToRoot, watchSystemTheme } from "./applyTheme";
import { assertBootstrapRanBeforeModule } from "./assertBootstrap";

afterEach(() => {
  const el = document.documentElement;
  delete el.dataset.theme;
  delete el.dataset.themePref;
  delete el.dataset.accent;
  delete el.dataset.translucency;
  delete el.dataset.themeBootstrapped;
  vi.restoreAllMocks();
  // @ts-expect-error — test-only cleanup of a jsdom global that isn't
  // implemented by default.
  delete window.matchMedia;
  // Tear down whatever module-level system-theme listener the last test left
  // behind, so it can't fire against a stale `el` in a later test. Switching
  // to a non-"system" theme is exactly what applyAppearanceToRoot itself uses
  // to unsubscribe (see its idempotent-listener JSDoc).
  applyAppearanceToRoot(el, { theme: "dark", accent: "indigo", translucency: true });
  delete el.dataset.theme;
  delete el.dataset.themePref;
  delete el.dataset.accent;
  delete el.dataset.translucency;
});

/**
 * Minimal MediaQueryList mock with a controllable `.matches` and a fake
 * "change" event bus, so tests can flip the simulated OS theme live and
 * assert `watchSystemTheme`/`applyAppearanceToRoot` react to it.
 */
function mockMatchMedia(initialPrefersDark: boolean) {
  let matches = initialPrefersDark;
  const listeners = new Set<() => void>();
  const mql = {
    get matches() {
      return matches;
    },
    media: "(prefers-color-scheme: dark)",
    addEventListener: vi.fn((_event: string, cb: () => void) => {
      listeners.add(cb);
    }),
    removeEventListener: vi.fn((_event: string, cb: () => void) => {
      listeners.delete(cb);
    }),
  };
  window.matchMedia = vi.fn().mockReturnValue(mql) as unknown as typeof window.matchMedia;
  return {
    setPrefersDark(next: boolean) {
      matches = next;
      listeners.forEach((cb) => cb());
    },
    listenerCount: () => listeners.size,
  };
}

describe("applyAppearanceToRoot", () => {
  it("writes the three data-* attributes with the on/off translucency mapping, and mirrors the raw choice into data-theme-pref", () => {
    const el = document.documentElement;
    applyAppearanceToRoot(el, { theme: "light", accent: "teal", translucency: false });
    expect(el.dataset.theme).toBe("light");
    expect(el.dataset.themePref).toBe("light");
    expect(el.dataset.accent).toBe("teal");
    expect(el.dataset.translucency).toBe("off");

    applyAppearanceToRoot(el, { theme: "dark", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("dark");
    expect(el.dataset.themePref).toBe("dark");
    expect(el.dataset.accent).toBe("indigo");
    expect(el.dataset.translucency).toBe("on");
  });

  it('resolves theme "system" to the OS-preferred dark/light for data-theme, keeping data-theme-pref="system"', () => {
    const el = document.documentElement;
    mockMatchMedia(true);
    applyAppearanceToRoot(el, { theme: "system", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("dark");
    expect(el.dataset.themePref).toBe("system");

    mockMatchMedia(false);
    applyAppearanceToRoot(el, { theme: "system", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("light");
    expect(el.dataset.themePref).toBe("system");
  });

  it('resolves theme "system" to "dark" when matchMedia is unavailable', () => {
    const el = document.documentElement;
    // @ts-expect-error — simulate an environment without matchMedia.
    delete window.matchMedia;
    applyAppearanceToRoot(el, { theme: "system", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("dark");
    expect(el.dataset.themePref).toBe("system");
  });

  it("live-updates data-theme when the OS theme changes while the choice is \"system\"", () => {
    const el = document.documentElement;
    const media = mockMatchMedia(true);
    applyAppearanceToRoot(el, { theme: "system", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("dark");

    media.setPrefersDark(false);
    expect(el.dataset.theme).toBe("light");

    media.setPrefersDark(true);
    expect(el.dataset.theme).toBe("dark");
  });

  it("manages the matchMedia listener idempotently: re-applying \"system\" doesn't stack listeners, and switching away tears it down", () => {
    const el = document.documentElement;
    const media = mockMatchMedia(true);

    applyAppearanceToRoot(el, { theme: "system", accent: "indigo", translucency: true });
    applyAppearanceToRoot(el, { theme: "system", accent: "teal", translucency: true });
    expect(media.listenerCount()).toBe(1);

    applyAppearanceToRoot(el, { theme: "dark", accent: "teal", translucency: true });
    expect(media.listenerCount()).toBe(0);

    // A stale OS-theme flip after switching away from "system" must NOT touch
    // data-theme anymore — the listener was torn down.
    media.setPrefersDark(false);
    expect(el.dataset.theme).toBe("dark");
  });
});

describe("watchSystemTheme", () => {
  it("subscribes and unsubscribes cleanly", () => {
    const el = document.createElement("div");
    const media = mockMatchMedia(true);
    el.dataset.theme = "dark";

    const unsubscribe = watchSystemTheme(el);
    expect(media.listenerCount()).toBe(1);

    media.setPrefersDark(false);
    expect(el.dataset.theme).toBe("light");

    unsubscribe();
    expect(media.listenerCount()).toBe(0);
    media.setPrefersDark(true);
    expect(el.dataset.theme).toBe("light"); // unchanged — unsubscribed
  });

  it("returns a no-op unsubscribe when matchMedia is unavailable", () => {
    const el = document.createElement("div");
    // @ts-expect-error — simulate an environment without matchMedia.
    delete window.matchMedia;
    const unsubscribe = watchSystemTheme(el);
    expect(() => unsubscribe()).not.toThrow();
  });
});

describe("assertBootstrapRanBeforeModule", () => {
  it("returns true when the ordering marker is present", () => {
    document.documentElement.dataset.themeBootstrapped = "1";
    expect(assertBootstrapRanBeforeModule("main")).toBe(true);
  });

  it("returns false when the marker is absent (and warns in DEV)", () => {
    delete document.documentElement.dataset.themeBootstrapped;
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(assertBootstrapRanBeforeModule("popup")).toBe(false);
    if (import.meta.env.DEV) expect(warn).toHaveBeenCalled();
  });
});
