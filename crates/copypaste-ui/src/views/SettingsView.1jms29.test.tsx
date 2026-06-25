/**
 * CopyPaste-1jms.29: notification-permission check must use the Tauri
 * check_notification_permission command, not browser Notification.permission.
 *
 * In a Tauri WKWebView the browser Notification API is a separate permission
 * system from macOS UNUserNotificationCenter, so Notification.permission does
 * not reflect macOS notification state. The wrapper isNotificationPermissionGranted()
 * already exists in lib/notificationPermission.ts and is the correct signal.
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
// Daemon stub that reports notify_on_copy: true
// ---------------------------------------------------------------------------

function makeOnlineInvoke(notifyOnCopy = true, notifPermGranted = false) {
  return (cmd: string, args: { method?: string; params?: unknown }) => {
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");
    if (cmd === "check_accessibility_permission") return Promise.resolve(true);
    // CopyPaste-1jms.29: check_notification_permission must be the source of truth.
    if (cmd === "check_notification_permission") return Promise.resolve(notifPermGranted);

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

describe("CopyPaste-1jms.29: notification permission uses Tauri command, not browser API", () => {
  it("shows warning when check_notification_permission returns false and notify is enabled", async () => {
    // check_notification_permission returns false = macOS denied.
    // Notification.permission is deliberately NOT stubbed to "denied" — the
    // implementation must NOT rely on it.
    invoke.mockImplementation(makeOnlineInvoke(true, false));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Warning must appear based on the Tauri command result, not browser API.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/notification.*denied|denied.*notification|notification.*permission.*denied|os.*notification/i);
    });

    // The invoke must have been called with "check_notification_permission".
    expect(invoke).toHaveBeenCalledWith("check_notification_permission");
  });

  it("does NOT show warning when check_notification_permission returns true", async () => {
    // macOS permission is granted — no warning.
    invoke.mockImplementation(makeOnlineInvoke(true, true));

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

  it("calls check_notification_permission command rather than browser Notification.permission", async () => {
    // Stub browser Notification as "granted" while the Tauri command says false (denied).
    // The warning must still appear because the Tauri command is authoritative.
    vi.stubGlobal("Notification", { permission: "granted" });
    invoke.mockImplementation(makeOnlineInvoke(true, false));

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Even though browser says "granted", the Tauri command says denied → warning.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/notification.*denied|denied.*notification|notification.*permission.*denied|os.*notification/i);
    });
  });
});
