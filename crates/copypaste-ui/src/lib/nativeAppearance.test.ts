import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { applyNativeAppearance } from "./nativeAppearance";

afterEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  invoke.mockReset();
});

describe("native appearance", () => {
  it("does nothing in a browser preview", () => {
    applyNativeAppearance("light");

    expect(invoke).not.toHaveBeenCalled();
  });

  it("synchronises the resolved theme with native chrome", () => {
    (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    invoke.mockResolvedValue(undefined);

    applyNativeAppearance("light");

    expect(invoke).toHaveBeenCalledWith("set_native_theme", { theme: "light" });
  });
});
