/**
 * M1 lazy-popup tests:
 *
 * 1. window.__copypasteFreeMemory is registered after Popup mounts.
 * 2. Calling it clears the ImageThumb LRU cache (calls clearImageCache).
 * 3. Calling it resets the Popup items list to empty
 *    (confirmed via disappearance of rendered row text).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import {
  clearImageCache,
  __testOnly_cacheSet,
  __testOnly_cacheSize,
} from "../components/ImageThumb";

// ---------------------------------------------------------------------------
// Tauri mocks — identical to popup.fixes.test.tsx
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
// Helper: minimal invoke mock that returns one history item.
// ---------------------------------------------------------------------------

function mockInvoke(items: Array<{ id: string; preview: string }>) {
  invoke.mockImplementation((cmd: string, args: unknown) => {
    if (cmd === "ipc_call") {
      const { method } = args as { method: string };
      if (method === "history_page") {
        return Promise.resolve({
          ok: true,
          data: {
            items: items.map((i) => ({
              id: i.id,
              content_type: "text",
              preview: i.preview,
              is_sensitive: false,
              wall_time: Date.now(),
              pinned: false,
            })),
            total: items.length,
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
// Declare the global hook type so TypeScript is happy in tests.
// ---------------------------------------------------------------------------

declare global {
  interface Window {
    __copypasteFreeMemory?: () => void;
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("M1: window.__copypasteFreeMemory hook", () => {
  beforeEach(() => {
    invoke.mockReset();
    // Remove the hook from any previous test's render.
    delete window.__copypasteFreeMemory;
    // Reset module-level cache so tests don't bleed into each other.
    clearImageCache();
  });

  it("is registered on window after Popup mounts", async () => {
    mockInvoke([]);
    const { Popup } = await import("./Popup");
    render(<Popup />);

    await waitFor(() => {
      expect(typeof window.__copypasteFreeMemory).toBe("function");
    });
  });

  it("calling it clears the ImageThumb LRU cache", async () => {
    mockInvoke([]);
    // Pre-populate the cache with a synthetic entry.
    __testOnly_cacheSet("synthetic-id", "data:image/png;base64,abc");
    expect(__testOnly_cacheSize()).toBe(1);

    const { Popup } = await import("./Popup");
    render(<Popup />);

    // Wait for hook to be registered.
    await waitFor(() => {
      expect(typeof window.__copypasteFreeMemory).toBe("function");
    });

    act(() => {
      window.__copypasteFreeMemory!();
    });

    expect(__testOnly_cacheSize()).toBe(0);
  });

  it("calling it empties the rendered items list", async () => {
    mockInvoke([{ id: "a", preview: "hello world" }]);
    const { Popup } = await import("./Popup");
    render(<Popup />);

    // Wait for the item to appear.
    await waitFor(() => {
      expect(screen.getByText("hello world")).toBeInTheDocument();
    });

    // Now call the free hook.
    act(() => {
      window.__copypasteFreeMemory!();
    });

    // Items list must be empty — row text gone, empty-state shown instead.
    await waitFor(() => {
      expect(screen.queryByText("hello world")).not.toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// clearImageCache smoke test (no Popup rendering needed)
// ---------------------------------------------------------------------------

describe("clearImageCache", () => {
  it("is exported from ImageThumb and can be called without error", () => {
    __testOnly_cacheSet("x", "data:image/png;base64,foo");
    expect(__testOnly_cacheSize()).toBe(1);
    clearImageCache();
    expect(__testOnly_cacheSize()).toBe(0);
  });
});
