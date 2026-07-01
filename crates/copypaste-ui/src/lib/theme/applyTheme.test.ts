import { afterEach, describe, expect, it, vi } from "vitest";
import { applyAppearanceToRoot } from "./applyTheme";
import { assertBootstrapRanBeforeModule } from "./assertBootstrap";

afterEach(() => {
  const el = document.documentElement;
  delete el.dataset.theme;
  delete el.dataset.accent;
  delete el.dataset.translucency;
  delete el.dataset.themeBootstrapped;
  vi.restoreAllMocks();
});

describe("applyAppearanceToRoot", () => {
  it("writes the three data-* attributes with the on/off translucency mapping", () => {
    const el = document.documentElement;
    applyAppearanceToRoot(el, { theme: "light", accent: "teal", translucency: false });
    expect(el.dataset.theme).toBe("light");
    expect(el.dataset.accent).toBe("teal");
    expect(el.dataset.translucency).toBe("off");

    applyAppearanceToRoot(el, { theme: "dark", accent: "indigo", translucency: true });
    expect(el.dataset.theme).toBe("dark");
    expect(el.dataset.accent).toBe("indigo");
    expect(el.dataset.translucency).toBe("on");
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
