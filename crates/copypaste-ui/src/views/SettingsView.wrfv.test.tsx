/**
 * CopyPaste-wrfv: When "Auto-apply synced clipboard" is enabled, the UI must
 * surface a notice that incoming synced clips will overwrite the current
 * clipboard content silently, so the user makes an informed decision.
 *
 * The actual overwrite is performed by the daemon (daemon-side). This test
 * only covers the UI surface: the notice must be visible when the toggle is ON.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";

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
// Daemon stub — auto_apply_synced_clip: true (default daemon behaviour)
// ---------------------------------------------------------------------------

function makeOnlineInvoke(autoApply = true) {
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
            relay_url: null, sync_enabled: true, sync_on_wifi_only: false,
            lan_visibility: true, auto_apply_synced_clip: autoApply,
            collect_public_ip: false, paste_as_plain_text: false,
            excluded_apps: [], max_text_size_bytes: null,
            max_image_size_bytes: null, max_file_size_bytes: null,
            storage_quota_bytes: null, sensitive_ttl_secs: null,
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
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CopyPaste-wrfv: Auto-apply synced clipboard notice", () => {
  it("shows a visible inline notice that auto-apply overwrites the active clipboard when ON", async () => {
    invoke.mockImplementation(makeOnlineInvoke(true));
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Navigate to the Sync tab where the auto-apply toggle lives.
    const syncTab = await screen.findByRole("tab", { name: /sync/i });
    fireEvent.click(syncTab);

    // A visible inline notice (not just a hidden InfoPopover tooltip) must warn
    // that auto-apply silently replaces the active clipboard content.
    await waitFor(() => {
      // Look for a dedicated warning/notice element, not just the toggle label.
      const alerts = screen.queryAllByRole("note");
      const notices = document.querySelectorAll("[data-testid='auto-apply-notice']");
      const hasNotice = alerts.length > 0 || notices.length > 0;
      expect(hasNotice).toBe(true);
    });
  });

  it("the auto-apply notice mentions clipboard overwrite risk", async () => {
    invoke.mockImplementation(makeOnlineInvoke(true));
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    const syncTab = await screen.findByRole("tab", { name: /sync/i });
    fireEvent.click(syncTab);

    // The notice must be locatable by testid and describe the overwrite risk.
    const notice = await screen.findByTestId("auto-apply-notice");
    expect(notice.textContent).toMatch(/overwrite|replac.*clipboard|clipboard.*overwrite|active clipboard/i);
  });

  it("notice is not a showstopper — the toggle can still be changed", async () => {
    invoke.mockImplementation(makeOnlineInvoke(true));
    invoke.mockImplementation((cmd: string, args?: unknown): Promise<unknown> => {
      if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "app_version") return Promise.resolve("0.7.5");
      if (cmd === "check_accessibility_permission") return Promise.resolve(true);
      const method = (args as { method?: string } | undefined)?.method;
      if (method === "status") return Promise.resolve({ ok: true, data: { ready: true, degraded: false, build_version: "0.7.5" }, error: null, error_code: null });
      if (method === "get_config") return Promise.resolve({ ok: true, data: { p2p_enabled: true, supabase_url: null, supabase_anon_key: null, relay_url: null, sync_enabled: true, sync_on_wifi_only: false, lan_visibility: true, auto_apply_synced_clip: true, collect_public_ip: false, paste_as_plain_text: false, excluded_apps: [], max_text_size_bytes: null, max_image_size_bytes: null, max_file_size_bytes: null, storage_quota_bytes: null, sensitive_ttl_secs: null }, error: null, error_code: null });
      if (method === "get_private_mode") return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      if (method === "get_sync_status") return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
      if (method === "set_config") return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    });

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    const syncTab = await screen.findByRole("tab", { name: /sync/i });
    fireEvent.click(syncTab);

    // The auto-apply toggle must be present and accessible.
    const autoApplyToggle = await screen.findByRole("switch", { name: /auto.apply synced clipboard/i });
    expect(autoApplyToggle).toBeInTheDocument();

    // Toggle can be clicked (notice doesn't block interaction).
    await act(async () => {
      fireEvent.click(autoApplyToggle);
    });
    // No crash = success for this test.
    expect(autoApplyToggle).toBeInTheDocument();
  });
});
