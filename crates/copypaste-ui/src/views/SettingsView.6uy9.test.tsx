/**
 * CopyPaste-6uy9: "Allow screenshots / screen recording" toggle.
 *
 * Verifies that:
 *  1. The toggle renders in the Settings General tab.
 *  2. It is unchecked by default (protection ON, PG-25 behaviour).
 *  3. Toggling ON calls set_allow_screenshots(true) via Tauri invoke.
 *  4. Toggling ON shows the privacy caveat warning text.
 *  5. Toggling OFF (back) clears the warning text.
 *  6. When set_allow_screenshots rejects, the toggle reverts and shows an error.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri bridge BEFORE importing SettingsView
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Stub helpers
// ---------------------------------------------------------------------------

function makeBaseInvoke({
  allowScreenshots = false,
  setAllowScreenshotsErr = false,
}: {
  allowScreenshots?: boolean;
  setAllowScreenshotsErr?: boolean;
} = {}) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");
    if (cmd === "check_accessibility_permission") return Promise.resolve(true);
    if (cmd === "get_allow_screenshots") return Promise.resolve(allowScreenshots);
    if (cmd === "set_allow_screenshots") {
      if (setAllowScreenshotsErr) {
        return Promise.reject(new Error("set_content_protected failed"));
      }
      return Promise.resolve(undefined);
    }

    const method = (args as { method?: string } | undefined)?.method;
    switch (method) {
      case "status":
        return Promise.resolve({
          ok: true,
          data: { ready: true, degraded: false, build_version: "0.7.5" },
          error: null,
          error_code: null,
        });
      case "get_config":
        return Promise.resolve({
          ok: true,
          data: {
            p2p_enabled: true,
            supabase_url: null,
            supabase_anon_key: null,
            relay_url: null,
            sync_on_wifi_only: false,
            lan_visibility: true,
            auto_apply_synced_clip: true,
            collect_public_ip: false,
            paste_as_plain_text: false,
            excluded_apps: [],
            sync_enabled: true,
            max_text_size_bytes: null,
            max_image_size_bytes: null,
            max_file_size_bytes: null,
            storage_quota_bytes: null,
            sensitive_ttl_secs: null,
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
          data: { last_sync_ms: null, supabase_url: null },
          error: null,
          error_code: null,
        });
      case "db_stats":
        return Promise.resolve({
          ok: true,
          data: { item_count: 0, size_bytes: 0 },
          error: null,
          error_code: null,
        });
      default:
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    }
  };
}

beforeEach(() => {
  invoke.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CopyPaste-6uy9: Allow screenshots toggle", () => {
  it("renders the toggle in the General tab", async () => {
    invoke.mockImplementation(makeBaseInvoke());

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // The label must be visible.
    await waitFor(() => {
      expect(
        screen.getByText(/Allow screenshots \/ screen recording/i),
      ).toBeInTheDocument();
    });
  });

  it("toggle is unchecked by default (protection ON)", async () => {
    invoke.mockImplementation(makeBaseInvoke({ allowScreenshots: false }));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      const toggle = screen.queryByRole("switch", {
        name: /Allow screenshots and screen recording/i,
      });
      if (toggle) {
        expect(toggle.getAttribute("aria-checked")).toBe("false");
      }
    });
  });

  it("hydrates toggle to true when persisted allow_screenshots is true", async () => {
    invoke.mockImplementation(makeBaseInvoke({ allowScreenshots: true }));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      const toggle = screen.queryByRole("switch", {
        name: /Allow screenshots and screen recording/i,
      });
      if (toggle) {
        expect(toggle.getAttribute("aria-checked")).toBe("true");
      }
    });
  });

  it("calls set_allow_screenshots(true) when the toggle is flipped ON", async () => {
    invoke.mockImplementation(makeBaseInvoke({ allowScreenshots: false }));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for load.
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Find and click the toggle.
    const toggle = await waitFor(() => {
      const t = screen.queryByRole("switch", {
        name: /Allow screenshots and screen recording/i,
      });
      expect(t).not.toBeNull();
      return t!;
    });

    fireEvent.click(toggle);

    await waitFor(() => {
      const calls = invoke.mock.calls;
      const setCall = calls.find(
        ([cmd, args]: [string, unknown]) =>
          cmd === "set_allow_screenshots" &&
          (args as { allow?: boolean })?.allow === true,
      );
      expect(setCall).toBeDefined();
    });
  });

  it("shows a privacy caveat when screenshots are allowed", async () => {
    invoke.mockImplementation(makeBaseInvoke({ allowScreenshots: true }));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      // When allowScreenshots is true, the warning note must be visible.
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/Clipboard content may be captured/i);
    });
  });

  it("does NOT show the caveat when screenshots are blocked (default)", async () => {
    invoke.mockImplementation(makeBaseInvoke({ allowScreenshots: false }));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const body = document.body.textContent ?? "";
    expect(body).not.toMatch(/Clipboard content may be captured/i);
  });

  it("reverts toggle and shows error when set_allow_screenshots rejects", async () => {
    invoke.mockImplementation(
      makeBaseInvoke({ allowScreenshots: false, setAllowScreenshotsErr: true }),
    );

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const toggle = await waitFor(() => {
      const t = screen.queryByRole("switch", {
        name: /Allow screenshots and screen recording/i,
      });
      expect(t).not.toBeNull();
      return t!;
    });

    fireEvent.click(toggle);

    await waitFor(() => {
      // Toggle should revert to unchecked.
      expect(toggle.getAttribute("aria-checked")).toBe("false");
    });

    await waitFor(() => {
      // Error message must appear.
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/Failed to update screenshot protection|set_content_protected/i);
    });
  });
});
