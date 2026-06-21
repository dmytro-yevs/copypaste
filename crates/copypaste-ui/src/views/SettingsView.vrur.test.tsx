/**
 * CopyPaste-vrur: When "Show notification on copy" is enabled but the OS
 * notification permission is denied, SettingsView must surface an in-app
 * warning so the user understands why notifications are silently missing.
 *
 * Strategy: mock `Notification.permission` to "denied" (OS-denied) and assert
 * that a warning is visible in the General tab when notifyOnCopy is true.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri bridge BEFORE importing any module that pulls in ipc.ts.
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
// Daemon stub that reports notify_on_copy: true (enabled)
// ---------------------------------------------------------------------------

function makeOnlineInvoke(notifyOnCopy = true) {
  return (cmd: string, args: { method?: string; params?: unknown }) => {
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
            relay_url: null, sync_enabled: true, sync_on_wifi_only: false,
            lan_visibility: true, auto_apply_synced_clip: true,
            collect_public_ip: false, paste_as_plain_text: false,
            excluded_apps: [], max_text_size_bytes: null,
            max_image_size_bytes: null, max_file_size_bytes: null,
            storage_quota_bytes: null, sensitive_ttl_secs: null, image_quality: null,
            notify_on_copy: notifyOnCopy,
          },
          error: null, error_code: null,
        });
      case "get_private_mode":
        return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      case "get_sync_status":
        return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
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
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CopyPaste-vrur: notification permission denial warning in SettingsView", () => {
  it("shows a warning when notify_on_copy is enabled but OS permission is denied", async () => {
    // Simulate OS-denied notification permission.
    vi.stubGlobal("Notification", { permission: "denied" });
    invoke.mockImplementation(makeOnlineInvoke(true));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for the view to load (daemon replies arrive).
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // A warning about denied notification permission must be visible.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/notification.*denied|denied.*notification|notification.*permission.*denied|os.*notification/i);
    });
  });

  it("does NOT show the warning when notify_on_copy is disabled", async () => {
    vi.stubGlobal("Notification", { permission: "denied" });
    invoke.mockImplementation(makeOnlineInvoke(false));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // When notifications are disabled, don't show the warning — it's not relevant.
    const body = document.body.textContent ?? "";
    expect(body).not.toMatch(/notification.*denied|denied.*notification/i);
  });

  it("does NOT show the warning when notification permission is granted", async () => {
    vi.stubGlobal("Notification", { permission: "granted" });
    invoke.mockImplementation(makeOnlineInvoke(true));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const body = document.body.textContent ?? "";
    expect(body).not.toMatch(/notification.*permission.*denied|os.*denied.*notification/i);
  });
});
