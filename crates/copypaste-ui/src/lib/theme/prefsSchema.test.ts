import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ACCENT_VALUES,
  DEFAULT_ACCENT,
  DEFAULT_THEME,
  DEFAULT_TRANSLUCENCY,
  THEME_VALUES,
  resolveTheme,
  translucencyAttr,
  validateAccent,
  validateTheme,
  validateTranslucency,
} from "./prefsSchema";

afterEach(() => {
  vi.restoreAllMocks();
  // @ts-expect-error — test-only cleanup of a jsdom global that isn't
  // implemented by default (see matchMedia mocks below).
  delete window.matchMedia;
});

/** Minimal MediaQueryList mock — enough for resolveTheme()'s `.matches` read. */
function mockMatchMedia(prefersDark: boolean): void {
  window.matchMedia = vi.fn().mockReturnValue({
    matches: prefersDark,
    media: "(prefers-color-scheme: dark)",
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }) as unknown as typeof window.matchMedia;
}

describe("prefsSchema constants", () => {
  it("declares the documented defaults and enum sizes", () => {
    expect(DEFAULT_THEME).toBe("dark");
    expect(DEFAULT_ACCENT).toBe("indigo");
    expect(DEFAULT_TRANSLUCENCY).toBe(true);
    expect(THEME_VALUES).toEqual(["system", "dark", "light"]);
    expect(ACCENT_VALUES).toHaveLength(6);
  });
});

describe("translucencyAttr", () => {
  it("maps boolean → on/off", () => {
    expect(translucencyAttr(true)).toBe("on");
    expect(translucencyAttr(false)).toBe("off");
  });
});

describe("validators — silent on absent, warn+default on invalid", () => {
  it("undefined defaults silently (no warn — field simply absent)", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(validateTheme(undefined)).toBe(DEFAULT_THEME);
    expect(validateAccent(undefined)).toBe(DEFAULT_ACCENT);
    expect(validateTranslucency(undefined)).toBe(DEFAULT_TRANSLUCENCY);
    expect(warn).not.toHaveBeenCalled();
  });

  it("valid values pass through", () => {
    for (const t of THEME_VALUES) expect(validateTheme(t)).toBe(t);
    for (const a of ACCENT_VALUES) expect(validateAccent(a)).toBe(a);
    expect(validateTranslucency(false)).toBe(false);
    expect(validateTranslucency(true)).toBe(true);
  });

  it("present-but-invalid values warn and default", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(validateTheme("sepia")).toBe(DEFAULT_THEME);
    expect(validateAccent("chartreuse")).toBe(DEFAULT_ACCENT);
    expect(validateTranslucency("yes")).toBe(DEFAULT_TRANSLUCENCY);
    expect(validateTranslucency(0)).toBe(DEFAULT_TRANSLUCENCY);
    expect(warn).toHaveBeenCalledTimes(4);
  });

  it("'system' is a valid theme choice (CopyPaste-g27b.20)", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(validateTheme("system")).toBe("system");
    expect(warn).not.toHaveBeenCalled();
  });
});

describe("resolveTheme", () => {
  it("passes dark/light through unchanged", () => {
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("light")).toBe("light");
  });

  it('resolves "system" via matchMedia("(prefers-color-scheme: dark)")', () => {
    mockMatchMedia(true);
    expect(resolveTheme("system")).toBe("dark");

    mockMatchMedia(false);
    expect(resolveTheme("system")).toBe("light");
  });

  it('resolves "system" to "dark" when matchMedia is unavailable (SSR guard)', () => {
    // @ts-expect-error — simulate an environment without matchMedia.
    delete window.matchMedia;
    expect(resolveTheme("system")).toBe("dark");
  });

  it('resolves "system" to "dark" when matchMedia throws', () => {
    window.matchMedia = vi.fn(() => {
      throw new Error("boom");
    }) as unknown as typeof window.matchMedia;
    expect(resolveTheme("system")).toBe("dark");
  });
});
