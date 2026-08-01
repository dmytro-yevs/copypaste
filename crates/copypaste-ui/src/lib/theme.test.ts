import { afterEach, describe, expect, it, vi } from "vitest";

import {
  _resetSystemThemeSubscription,
  applyAppearance,
  subscribeSystemTheme,
} from "./theme";

const originalMatchMedia = Object.getOwnPropertyDescriptor(window, "matchMedia");

afterEach(() => {
  _resetSystemThemeSubscription();
  vi.restoreAllMocks();
  if (originalMatchMedia) {
    Object.defineProperty(window, "matchMedia", originalMatchMedia);
  } else {
    delete (window as { matchMedia?: typeof window.matchMedia }).matchMedia;
  }
});

describe("system appearance after startup (AT-53)", () => {
  it("updates live through exactly one media-query listener", () => {
    let matches = false;
    let onChange: (() => void) | undefined;
    const addEventListener = vi.fn((_event: string, listener: () => void) => {
      onChange = listener;
    });
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn(() => ({ matches, addEventListener }) as unknown as MediaQueryList),
    });
    const prefs = { theme: "system", accent: "blue", translucency: 0 } as const;

    applyAppearance(prefs);
    subscribeSystemTheme(() => applyAppearance(prefs));
    subscribeSystemTheme(() => applyAppearance(prefs));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(addEventListener).toHaveBeenCalledTimes(1);

    matches = true;
    onChange?.();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("applies translucency intensity as CSS variables", () => {
    applyAppearance({ theme: "dark", accent: "indigo", translucency: 50 });

    expect(document.documentElement.dataset.translucency).toBe("on");
    expect(document.documentElement.style.getPropertyValue("--translucency-blur")).toBe("10px");
    expect(document.documentElement.style.getPropertyValue("--translucency-chrome-opacity")).toBe("86%");

    applyAppearance({ theme: "dark", accent: "indigo", translucency: 0 });
    expect(document.documentElement.dataset.translucency).toBe("off");
  });

  it("keeps the browser-preview fallback when System accent is selected", () => {
    applyAppearance({ theme: "dark", accent: "system", translucency: 0 });

    expect(document.documentElement.dataset.accent).toBe("system");
    expect(document.documentElement.style.getPropertyValue("--accent")).toBe("");
  });
});
