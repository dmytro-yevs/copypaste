/**
 * Phase 4: Popup uses fixed design tokens (--r-card).
 * Updated from W-C5: legacy skin system removed; two-axis (theme × accent) only.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor, act } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onFocusChanged: vi.fn().mockResolvedValue(() => {}),
    hide: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: {
    getByLabel: vi.fn().mockResolvedValue(null),
  },
}));

function mockEmptyHistory() {
  invoke.mockImplementation((cmd: string, args: unknown) => {
    if (cmd === "ipc_call") {
      const { method } = (args as { method: string });
      if (method === "history_page") {
        return Promise.resolve({
          ok: true,
          data: { items: [], total: 0 },
          error: null,
          error_code: null,
        });
      }
    }
    return Promise.resolve(null);
  });
}

describe("Phase 4: Popup root uses var(--r-card) for borderRadius", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  it("popup root div has borderRadius: var(--r-card)", async () => {
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    const popupRoot = container.querySelector("[data-popup-root]") as HTMLElement | null;
    expect(popupRoot).not.toBeNull();
    expect(popupRoot!.style.borderRadius).toBe("var(--r-card)");
  });

  it("html element has only two-axis appearance attributes (theme + accent, no legacy axes)", async () => {
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    await act(async () => { render(<Popup />); });

    // Collect all data-* attributes set on <html>.
    const dataAttrs = Array.from(document.documentElement.attributes)
      .filter((a) => a.name.startsWith("data-"))
      .map((a) => a.name);

    // Only data-theme and data-accent are permitted (two-axis system, Phase 4).
    const unexpected = dataAttrs.filter(
      (a) => a !== "data-theme" && a !== "data-accent",
    );
    expect(unexpected).toEqual([]);
  });
});
