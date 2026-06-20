/**
 * Audit fix tests:
 *   CopyPaste-7rns (P1): popup window borderRadius must be var(--skin-r-card),
 *     not var(--skin-r-modal), so classic renders 14px (pre-skin byte-identical).
 *   CopyPaste-kp6f (W5, popup part): source-app label chip must use
 *     borderRadius: "var(--skin-r-chip)" inline style instead of static
 *     rounded-ide-sm Tailwind class.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri mocks
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
// CopyPaste-7rns: popup root borderRadius must use var(--skin-r-card)
// ---------------------------------------------------------------------------

describe("CopyPaste-7rns: popup root uses var(--skin-r-card) for borderRadius", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  it("popup root div borderRadius is var(--skin-r-card), not var(--skin-r-modal)", async () => {
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    const popupRoot = container.querySelector("[data-popup-root]") as HTMLElement | null;
    expect(popupRoot).not.toBeNull();

    const radius = popupRoot!.style.borderRadius;
    // Must reference the card token (14px classic, 10px quiet, 16px vapor).
    expect(radius).toBe("var(--skin-r-card)");
    // Must NOT reference the modal token (which gives 16px in classic — wrong).
    expect(radius).not.toBe("var(--skin-r-modal)");
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-kp6f: source-app chip uses inline borderRadius var(--skin-r-chip)
// ---------------------------------------------------------------------------

describe("CopyPaste-kp6f: source-app chip uses var(--skin-r-chip) inline style", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  it("source-app chip span has borderRadius: var(--skin-r-chip) inline style", async () => {
    // Use a known bundle ID that sourceAppLabel maps to a non-empty string.
    mockHistoryWithApp("com.apple.terminal");

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    // Wait for the item row to render (app label "Terminal" appears).
    await waitFor(() => {
      expect(screen.getByText("Terminal")).toBeInTheDocument();
    });

    // Find the chip span — it renders the app label text.
    const chip = screen.getByText("Terminal").closest("span");
    expect(chip).not.toBeNull();

    // Must have borderRadius set as inline style with the skin token.
    expect(chip!.style.borderRadius).toBe("var(--skin-r-chip)");

    // Must NOT rely on the static Tailwind class.
    expect(chip!.className).not.toContain("rounded-ide-sm");
  });
});
