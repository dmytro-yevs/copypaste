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
      value: vi.fn(
        () =>
          ({
            get matches() {
              return matches;
            },
            addEventListener,
          }) as unknown as MediaQueryList,
      ),
    });
    const prefs = {
      theme: "system",
      colorTheme: "aurora",
      translucency: false,
    } as const;

    applyAppearance(prefs);
    subscribeSystemTheme(() => applyAppearance(prefs));
    subscribeSystemTheme(() => applyAppearance(prefs));
    expect(document.documentElement.dataset.colorScheme).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("aurora");
    expect(document.documentElement.dataset.mode).toBe("system");
    expect(addEventListener).toHaveBeenCalledTimes(1);

    matches = true;
    onChange?.();
    expect(document.documentElement.dataset.colorScheme).toBe("dark");
  });

  it("applies the token-backed translucency state without inline design values", () => {
    applyAppearance({
      theme: "dark",
      colorTheme: "midnight",
      translucency: true,
    });

    expect(document.documentElement.dataset.translucency).toBe("on");
    expect(document.documentElement.style.length).toBe(0);

    applyAppearance({
      theme: "dark",
      colorTheme: "midnight",
      translucency: false,
    });
    expect(document.documentElement.dataset.translucency).toBe("off");
  });
});
