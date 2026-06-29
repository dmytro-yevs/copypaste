/**
 * Phase 4: Popup uses fixed design tokens (--r-card), no data-skin attribute.
 * Updated from W-C5: skin system removed.
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

  it("does NOT set data-skin attribute on <html> (skin system removed)", async () => {
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    await act(async () => { render(<Popup />); });

    expect(document.documentElement.hasAttribute("data-skin")).toBe(false);
  });
});
