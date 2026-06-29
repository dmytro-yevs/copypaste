/**
 * Phase 4: Popup uses fixed radius tokens (--r-card, --r-chip).
 * Updated from CopyPaste-7rns/kp6f audit: old skin tokens replaced.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

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
      const { method } = args as { method: string };
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

function mockHistoryWithApp(app_bundle_id: string) {
  invoke.mockImplementation((cmd: string, args: unknown) => {
    if (cmd === "ipc_call") {
      const { method } = args as { method: string };
      if (method === "history_page") {
        return Promise.resolve({
          ok: true,
          data: {
            items: [
              {
                id: "item1",
                content_type: "text",
                preview: "clipboard text",
                is_sensitive: false,
                wall_time: Date.now(),
                pinned: false,
                app_bundle_id,
              },
            ],
            total: 1,
          },
          error: null,
          error_code: null,
        });
      }
    }
    return Promise.resolve(null);
  });
}

// ---------------------------------------------------------------------------
// Popup root borderRadius uses var(--r-card)
// ---------------------------------------------------------------------------

describe("Phase 4: popup root uses var(--r-card) for borderRadius", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  it("popup root div borderRadius is var(--r-card)", async () => {
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    const popupRoot = container.querySelector("[data-popup-root]") as HTMLElement | null;
    expect(popupRoot).not.toBeNull();

    const radius = popupRoot!.style.borderRadius;
    expect(radius).toBe("var(--r-card)");
  });
});

// ---------------------------------------------------------------------------
// Source-app chip uses inline borderRadius var(--r-chip)
// ---------------------------------------------------------------------------

describe("Phase 4: source-app chip uses var(--r-chip) inline style", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  it("source-app chip span has borderRadius: var(--r-chip) inline style", async () => {
    mockHistoryWithApp("com.apple.terminal");

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    await waitFor(() => {
      expect(screen.getByText("Terminal")).toBeInTheDocument();
    });

    const chip = screen.getByText("Terminal").closest("span");
    expect(chip).not.toBeNull();

    expect(chip!.style.borderRadius).toBe("var(--r-chip)");
    expect(chip!.className).not.toContain("rounded-ide-sm");
  });
});
