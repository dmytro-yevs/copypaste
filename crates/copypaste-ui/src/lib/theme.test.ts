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
    const prefs = { theme: "system", accent: "blue", translucency: false } as const;

    applyAppearance(prefs);
    subscribeSystemTheme(() => applyAppearance(prefs));
    subscribeSystemTheme(() => applyAppearance(prefs));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(addEventListener).toHaveBeenCalledTimes(1);

    matches = true;
    onChange?.();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
