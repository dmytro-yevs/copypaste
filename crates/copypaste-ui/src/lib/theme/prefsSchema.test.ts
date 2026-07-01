import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ACCENT_VALUES,
  DEFAULT_ACCENT,
  DEFAULT_THEME,
  DEFAULT_TRANSLUCENCY,
  THEME_VALUES,
  translucencyAttr,
  validateAccent,
  validateTheme,
  validateTranslucency,
} from "./prefsSchema";

afterEach(() => vi.restoreAllMocks());

describe("prefsSchema constants", () => {
  it("declares the documented defaults and enum sizes", () => {
    expect(DEFAULT_THEME).toBe("dark");
    expect(DEFAULT_ACCENT).toBe("indigo");
    expect(DEFAULT_TRANSLUCENCY).toBe(true);
    expect(THEME_VALUES).toEqual(["dark", "light"]);
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
    expect(validateTheme("system")).toBe(DEFAULT_THEME);
    expect(validateAccent("chartreuse")).toBe(DEFAULT_ACCENT);
    expect(validateTranslucency("yes")).toBe(DEFAULT_TRANSLUCENCY);
    expect(validateTranslucency(0)).toBe(DEFAULT_TRANSLUCENCY);
    expect(warn).toHaveBeenCalledTimes(4);
  });
});
