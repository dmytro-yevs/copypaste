/**
 * Tests for self-contained audit fixes in the popup and related components.
 *
 * Fix #1: filter operates on the SAME normalized label that is rendered.
 * Fix #2: popup re-fetches on a short interval while visible (not only on focus/mount).
 * Fix #3: P2P toggle in SettingsView sends the full config payload.
 * Fix #5: DevicesView filters own fingerprint from peer list in render.
 * Fix #6: AboutView GitHub URL matches actual git remote.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Shared Tauri mock infrastructure
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

// ---------------------------------------------------------------------------
// Fix #1: Popup filter uses normalized label (not raw preview)
// ---------------------------------------------------------------------------

/**
 * The popup normalises item.preview with `.replace(/\s+/g," ").trim()` before
 * rendering. The filter must match against that same normalized string, not the
 * raw preview. If it matches the raw preview, a query with single spaces will
 * fail to match a preview that has multiple consecutive spaces at the same
 * positions.
 *
 * Example: preview = "hello   world" (3 spaces)
 *   normalized label = "hello world" (1 space)
 *   query "hello world" → should still show the row after filtering
 */
describe("Fix #1: popup label normalization consistent with filter", () => {
  it("normalized label matches a query that would not match the raw preview", () => {
    const rawPreview = "hello   world\n  foo";
    const normalizedLabel = rawPreview.replace(/\s+/g, " ").trim();
    const query = "hello world foo";
    // Filter must check against the normalized form.
    expect(normalizedLabel.toLowerCase().includes(query.toLowerCase())).toBe(true);
    // Sanity: raw preview does NOT match the same query.
    expect(rawPreview.toLowerCase().includes(query.toLowerCase())).toBe(false);
  });

  it("Popup keeps the row visible when the query matches the normalized label", async () => {
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
                  // Three spaces — normalized to one space in the label.
                  preview: "hello   world",
                  is_sensitive: false,
                  wall_time: Date.now(),
                  pinned: false,
                },
              ],
              total: 1,
            },
            error: null,
            error_code: null,
          });
        }
      }
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    const { Popup } = await import("./Popup");
    render(<Popup />);

    // Wait for the item to appear (normalized label).
    await waitFor(() => expect(screen.getByText("hello world")).toBeInTheDocument());

    // Type a query that only matches the normalized label (single space between words).
    const input = screen.getByPlaceholderText(/Search clipboard/i);
    fireEvent.change(input, { target: { value: "hello world" } });

    // Row must remain visible — filter ran against normalized label, not raw.
    await waitFor(() => expect(screen.getByText("hello world")).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Fix #2: Popup polls for new items while visible
// ---------------------------------------------------------------------------

describe("Fix #2: popup polls for new items while open", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    Object.defineProperty(document, "visibilityState", {
      value: "visible",
      configurable: true,
    });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("re-fetches at least once after the polling interval elapses", async () => {
    let callCount = 0;
    invoke.mockImplementation((cmd: string, args: unknown) => {
      if (cmd === "ipc_call") {
        const { method } = args as { method: string };
        if (method === "history_page") {
          callCount++;
          return Promise.resolve({
            ok: true,
            data: { items: [], total: 0 },
            error: null,
            error_code: null,
          });
        }
      }
      return Promise.reject(new Error(`unexpected: ${cmd}`));
    });

    const { Popup } = await import("./Popup");
    render(<Popup />);

    // Flush the initial load.
    await act(async () => {
      await Promise.resolve();
    });
    const callsAfterMount = callCount;

    // Advance 5 seconds — enough for any reasonable polling interval.
    await act(async () => {
      vi.advanceTimersByTime(5000);
      // Give promises triggered by the timer a chance to resolve.
      await Promise.resolve();
    });

    // Must have fetched at least once more after mount.
    expect(callCount).toBeGreaterThan(callsAfterMount);
  });
});

// ---------------------------------------------------------------------------
// Fix #3: SettingsView P2P toggle sends the full config payload
// ---------------------------------------------------------------------------

describe("Fix #3: SettingsView P2P toggle sends full config payload", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("toggling P2P calls set_config with all three AppSettings fields", async () => {
    invoke.mockImplementation((cmd: string, args: unknown) => {
      if (cmd === "ipc_call") {
        const { method } = args as { method: string };
        switch (method) {
          case "get_config":
            return Promise.resolve({
              ok: true,
              data: {
                p2p_enabled: false,
                supabase_url: "https://example.supabase.co",
                supabase_anon_key: "my-key",
              },
              error: null,
              error_code: null,
            });
          case "get_private_mode":
            return Promise.resolve({
              ok: true,
              data: { private_mode: false },
              error: null,
              error_code: null,
            });
          case "get_sync_status":
            return Promise.resolve({
              ok: true,
              data: {
                passphrase_set: false,
                supabase_configured: true,
                signed_in: false,
                last_sync_ms: null,
              },
              error: null,
              error_code: null,
            });
          case "set_config":
            return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
          default:
            return Promise.reject(new Error(`unexpected method: ${method}`));
        }
      }
      // Tauri-direct commands (get_popup_shortcut, etc.) return a sensible default.
      return Promise.resolve("CmdOrCtrl+Shift+V");
    });

    const { SettingsView } = await import("../views/SettingsView");
    render(<SettingsView />);

    // Wait for loading to finish.
    await waitFor(() => {
      expect(screen.queryByText(/^Loading…$/i)).not.toBeInTheDocument();
    });

    // Select the "Sync" tab (role="tab" after a11y fix CopyPaste-9c8)
    const syncTab = screen.getByRole("tab", { name: /Sync/i });
    fireEvent.click(syncTab);

    // Find the P2P toggle switch and click it.
    const p2pToggle = await screen.findByRole("switch", { name: /P2P sync/i });
    fireEvent.click(p2pToggle);

    // set_config must have been called with the FULL payload.
    await waitFor(() => {
      const setConfigCall = invoke.mock.calls.find(
        (c) =>
          c[0] === "ipc_call" &&
          (c[1] as { method: string }).method === "set_config"
      );
      expect(setConfigCall).toBeDefined();
      const payload = (setConfigCall![1] as { method: string; params: unknown })
        .params as Record<string, unknown>;
      expect(payload).toHaveProperty("p2p_enabled", true);
      expect(payload).toHaveProperty("supabase_url", "https://example.supabase.co");
      expect(payload).toHaveProperty("supabase_anon_key", "my-key");
    });
  });
});

// ---------------------------------------------------------------------------
// Fix #5: DevicesView filters own fingerprint from the peer list
// ---------------------------------------------------------------------------

describe("Fix #5: DevicesView does not show own device in peer list", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("own fingerprint peer row has no Unpair button", async () => {
    const OWN_FP = "aabbccddeeff0011";
    const OTHER_FP = "1122334455667788";

    invoke.mockImplementation((cmd: string, args: unknown) => {
      if (cmd === "ipc_call") {
        const { method } = args as { method: string };
        switch (method) {
          case "list_peers":
            return Promise.resolve({
              ok: true,
              data: {
                peers: [
                  { fingerprint: OWN_FP, name: "This Mac" },
                  { fingerprint: OTHER_FP, name: "iPhone" },
                ],
              },
              error: null,
              error_code: null,
            });
          case "get_own_fingerprint":
          case "get_own_device_info":
            return Promise.resolve({
              ok: true,
              data: {
                fingerprint: OWN_FP,
                device_name: "This Mac",
                device_model: "MacBookPro18,2",
                os_version: "macOS 14.5.0",
                app_version: "0.5.3",
                local_ip: "192.168.1.10",
              },
              error: null,
              error_code: null,
            });
          default:
            return Promise.reject(new Error(`unexpected: ${method}`));
        }
      }
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    const { DevicesView } = await import("../views/DevicesView");
    render(<DevicesView />);

    // Wait for both loads to settle.
    await waitFor(() => {
      expect(screen.queryByText(/Loading/i)).not.toBeInTheDocument();
    });

    // "iPhone" (other device) must be visible.
    expect(screen.getByText("iPhone")).toBeInTheDocument();

    // Only ONE Unpair button — for the other device.  Own device must be excluded.
    // Note: aria-label is now "Unpair <device-name>" (added by CopyPaste-wv57),
    // so match the Unpair prefix rather than an exact-text query.
    const unpairButtons = screen.queryAllByRole("button", { name: /^Unpair\b/i });
    expect(unpairButtons).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// Fix #6: AboutView GitHub URL matches real remote
// ---------------------------------------------------------------------------

describe("Fix #6: AboutView GitHub URL matches real remote", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("renders github.com/dmytro-yevs/copypaste, not the placeholder", async () => {
    invoke.mockImplementation((cmd: string, args: unknown) => {
      if (cmd === "ipc_call") {
        const { method } = args as { method: string };
        if (method === "status") {
          return Promise.resolve({ ok: true, data: {}, error: null, error_code: null });
        }
      }
      return Promise.reject(new Error(`unexpected: ${cmd}`));
    });

    const { AboutView } = await import("../views/AboutView");
    render(<AboutView />);

    await waitFor(() => {
      expect(
        screen.getByText(/github\.com\/dmytro-yevs\/copypaste/i)
      ).toBeInTheDocument();
    });

    expect(
      screen.queryByText(/github\.com\/dmytro\/CopyPaste/i)
    ).not.toBeInTheDocument();
  });
});
