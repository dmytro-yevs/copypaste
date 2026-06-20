/**
 * W-C5: Popup skin token tests.
 *
 * Verifies that the Popup:
 *   1. Syncs prefs.skin → data-skin on document.documentElement (mirrors how
 *      theme and translucency are synced).
 *   2. Uses var(--skin-r-modal) for border-radius instead of a hardcoded value.
 *   3. Keeps classic behaviour unchanged (existing Popup tests stay green).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri mocks — same pattern as popup.fixes.test.tsx
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
// Helper: minimal invoke that returns an empty history
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("W-C5: Popup syncs data-skin to <html>", () => {
  beforeEach(() => {
    invoke.mockReset();
    // Reset localStorage between tests so the store re-initialises cleanly.
    localStorage.clear();
    // Reset modules so each test gets a fresh store and a fresh Popup.
    vi.resetModules();
  });

  afterEach(() => {
    // Clean up any data-skin attribute left on the document element.
    document.documentElement.removeAttribute("data-skin");
  });

  it("sets data-skin='quiet' on <html> when prefs.skin is 'quiet'", async () => {
    // Arrange: persist quiet skin in localStorage so the store loads it.
    localStorage.setItem(
      "copypaste-ui-prefs-v3",
      JSON.stringify({
        previewLinesApp: 1,
        previewLinesPopup: 1,
        previewSize: 28,
        maskSensitive: true,
        imageMaxHeight: 40,
        playSoundOnCopy: true,
        notifyOnCopy: true,
        translucency: true,
        theme: "light",
        density: "compact",
        palette: "graphite-mist",
        motionReduced: false,
        historyDisplayLimit: 1000,
        showSensitiveWarnings: true,
        skin: "quiet",
      })
    );

    mockEmptyHistory();

    // Dynamic import so vi.resetModules() above takes effect on the store.
    const { Popup } = await import("./Popup");
    await act(async () => {
      render(<Popup />);
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-skin")).toBe("quiet");
    });
  });

  it("sets data-skin='vapor' on <html> when prefs.skin is 'vapor'", async () => {
    localStorage.setItem(
      "copypaste-ui-prefs-v3",
      JSON.stringify({
        previewLinesApp: 1,
        previewLinesPopup: 1,
        previewSize: 28,
        maskSensitive: true,
        imageMaxHeight: 40,
        playSoundOnCopy: true,
        notifyOnCopy: true,
        translucency: true,
        theme: "dark",
        density: "compact",
        palette: "graphite-mist",
        motionReduced: false,
        historyDisplayLimit: 1000,
        showSensitiveWarnings: true,
        skin: "vapor",
      })
    );

    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    await act(async () => {
      render(<Popup />);
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-skin")).toBe("vapor");
    });
  });

  it("defaults to data-skin='classic' when prefs.skin is 'classic'", async () => {
    localStorage.setItem(
      "copypaste-ui-prefs-v3",
      JSON.stringify({
        previewLinesApp: 1,
        previewLinesPopup: 1,
        previewSize: 28,
        maskSensitive: true,
        imageMaxHeight: 40,
        playSoundOnCopy: true,
        notifyOnCopy: true,
        translucency: true,
        theme: "light",
        density: "compact",
        palette: "graphite-mist",
        motionReduced: false,
        historyDisplayLimit: 1000,
        showSensitiveWarnings: true,
        skin: "classic",
      })
    );

    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    await act(async () => {
      render(<Popup />);
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-skin")).toBe("classic");
    });
  });
});

describe("W-C5 / CopyPaste-7rns: Popup root uses var(--skin-r-card) for borderRadius", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
    vi.resetModules();
  });

  afterEach(() => {
    document.documentElement.removeAttribute("data-skin");
  });

  it("popup root div has borderRadius: var(--skin-r-card)", async () => {
    // Default prefs (classic skin).
    mockEmptyHistory();

    const { Popup } = await import("./Popup");
    const { container } = render(<Popup />);

    // The popup root is data-popup-root — find it.
    const popupRoot = container.querySelector("[data-popup-root]") as HTMLElement | null;
    expect(popupRoot).not.toBeNull();

    // CopyPaste-7rns: must use --skin-r-card (classic=14px, pre-skin byte-identical),
    // NOT --skin-r-modal (which gives classic=16px, breaking byte-identity).
    // In jsdom, CSS variables are not resolved but the inline style value should
    // contain "var(--skin-r-card)".
    const radius = popupRoot!.style.borderRadius;
    expect(radius).toBe("var(--skin-r-card)");
  });
});
