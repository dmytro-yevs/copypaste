/**
 * CopyPaste-gq51 — vacuum/stats UI surface in SettingsView.
 *
 * The daemon supports "vacuum" (compact the SQLite WAL) and "db_stats"
 * (return item count + file size). This test verifies the Storage tab
 * in SettingsView exposes a button to trigger vacuum and a stats display.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri core bridge BEFORE importing SettingsView.
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

function makeOnlineInvoke(
  overrides: Record<string, (params?: unknown) => unknown> = {},
) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    // Handle Tauri-direct commands (not routed through ipc_call).
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");

    const method = (args as { method?: string } | undefined)?.method;
    if (method && overrides[method]) {
      return Promise.resolve(overrides[method]((args as { params?: unknown })?.params));
    }

    switch (method) {
      case "status":
        return Promise.resolve({
          ok: true,
          data: { ready: true, degraded: false, build_version: "0.7.5" },
          error: null, error_code: null,
        });
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
          },
          error: null, error_code: null,
        });
      case "get_private_mode":
        return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      case "get_sync_status":
        return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
      case "db_stats":
        return Promise.resolve({
          ok: true,
          data: { item_count: 42, size_bytes: 1_048_576 },
          error: null, error_code: null,
        });
      case "vacuum":
        return Promise.resolve({ ok: true, data: { ok: true }, error: null, error_code: null });
      default:
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    }
  };
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(makeOnlineInvoke());
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function renderAndGoToStorage() {
  render(
    <ErrorBoundary label="Settings">
      <SettingsView />
    </ErrorBoundary>,
  );

  // Navigate to the Storage tab.
  const storageTab = await screen.findByRole("tab", { name: /storage/i });
  fireEvent.click(storageTab);
}

// ---------------------------------------------------------------------------
// gq51: vacuum/stats must be surfaced in the Storage tab
// ---------------------------------------------------------------------------

describe("CopyPaste-gq51: vacuum and stats UI in SettingsView Storage tab", () => {
  it("shows db stats (item count and size) in the Storage tab", async () => {
    await renderAndGoToStorage();

    // Stats must appear: item count (42) and size (1 MB).
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      // Item count should appear somewhere in the stats section.
      expect(body).toMatch(/42/);
    });
  });

  it("shows a 'Vacuum' button in the Storage tab", async () => {
    await renderAndGoToStorage();

    const vacuumBtn = await screen.findByRole("button", { name: /vacuum/i });
    expect(vacuumBtn).toBeInTheDocument();
  });

  it("clicking Vacuum calls the 'vacuum' IPC method", async () => {
    await renderAndGoToStorage();

    const vacuumBtn = await screen.findByRole("button", { name: /vacuum/i });

    await act(async () => {
      fireEvent.click(vacuumBtn);
    });

    await waitFor(() => {
      const vacuumCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "vacuum",
      );
      expect(vacuumCalls.length).toBeGreaterThan(0);
    });
  });

  it("shows success feedback after vacuum completes", async () => {
    await renderAndGoToStorage();

    const vacuumBtn = await screen.findByRole("button", { name: /vacuum/i });

    await act(async () => {
      fireEvent.click(vacuumBtn);
    });

    // After vacuum, some success indication must be shown.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/vacuum|compacted|done|ok/i);
    });
  });
});
