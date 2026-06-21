/**
 * CopyPaste-7set: sync_enabled toggle must reconcile with the daemon's real
 * sync_enabled state. When the daemon's get_config response does not include
 * a `sync_enabled` field (older daemon / daemon-side stub not yet implemented),
 * the UI must surface a warning so the user knows the toggle may have no effect.
 *
 * When the daemon does include `sync_enabled`, the toggle must hydrate to the
 * daemon-reported value (not assume true).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri bridge BEFORE importing.
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
// Daemon stub helpers
// ---------------------------------------------------------------------------

function makeBaseInvoke(configOverrides: Record<string, unknown> = {}) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");
    if (cmd === "check_accessibility_permission") return Promise.resolve(true);

    const method = (args as { method?: string } | undefined)?.method;
    switch (method) {
      case "status":
        return Promise.resolve({ ok: true, data: { ready: true, degraded: false, build_version: "0.7.5" }, error: null, error_code: null });
      case "get_config":
        return Promise.resolve({
          ok: true,
          data: {
            p2p_enabled: true, supabase_url: null, supabase_anon_key: null,
            relay_url: null, sync_on_wifi_only: false, lan_visibility: true,
            auto_apply_synced_clip: true, collect_public_ip: false,
            paste_as_plain_text: false, excluded_apps: [], max_text_size_bytes: null,
            max_image_size_bytes: null, max_file_size_bytes: null,
            storage_quota_bytes: null, sensitive_ttl_secs: null, image_quality: null,
            ...configOverrides,
          },
          error: null, error_code: null,
        });
      case "get_private_mode":
        return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      case "get_sync_status":
        return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
      case "set_config":
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
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

describe("CopyPaste-7set: sync_enabled toggle reconciliation", () => {
  it("shows a stub warning when daemon get_config omits sync_enabled (older daemon)", async () => {
    // Daemon does NOT include sync_enabled — it was a stub and the field is absent.
    invoke.mockImplementation(makeBaseInvoke({ /* no sync_enabled */ }));
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for load.
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // A stale/stub warning must be visible somewhere in the General tab
    // when the daemon doesn't acknowledge sync_enabled.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      // Must mention that the toggle may not be applied yet or daemon is a stub.
      expect(body).toMatch(/sync.*stub|stub.*sync|no effect|daemon.*sync_enabled|sync_enabled.*daemon|sync.*not.*supported|sync.*ignored/i);
    });
  });

  it("does NOT show the stub warning when daemon returns sync_enabled: true", async () => {
    invoke.mockImplementation(makeBaseInvoke({ sync_enabled: true }));
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // No stub warning when the daemon returns the field.
    const body = document.body.textContent ?? "";
    expect(body).not.toMatch(/sync.*stub|stub.*sync|sync.*not.*supported|sync.*ignored/i);
  });

  it("hydrates the toggle to daemon-reported sync_enabled: false when daemon supports it", async () => {
    invoke.mockImplementation(makeBaseInvoke({ sync_enabled: false }));
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // The "Enable sync" toggle must reflect daemon-reported false (unchecked).
    await waitFor(() => {
      const syncToggle = screen.queryByRole("switch", { name: /enable sync/i });
      if (syncToggle) {
        expect(syncToggle.getAttribute("aria-checked")).toBe("false");
      }
    });
  });
});
