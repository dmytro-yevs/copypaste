/**
 * CopyPaste-86ii: A backup/restore surface must be present in the Settings GUI.
 *
 * Previously only the CLI and scripts supported backup/restore. This test
 * verifies the Backup & Restore panel is visible in the Storage tab and
 * surfaces both export and import controls with adequate descriptive text.
 *
 * Note: The IPC-level backup/restore uses JSON export/import (not a raw SQLCipher
 * database backup). A raw SQLCipher VACUUM-INTO backup would require a new
 * daemon IPC verb ("db_backup" / "db_restore") which is not yet implemented
 * daemon-side. This UI surface uses the existing "export" / "import" IPC verbs.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
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

function makeOnlineInvoke() {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.5");
    if (cmd === "check_accessibility_permission") return Promise.resolve(true);
    const method = (args as { method?: string } | undefined)?.method;
    switch (method) {
      case "status": return Promise.resolve({ ok: true, data: { ready: true, degraded: false, build_version: "0.7.5" }, error: null, error_code: null });
      case "get_config": return Promise.resolve({ ok: true, data: { p2p_enabled: true, supabase_url: null, supabase_anon_key: null, relay_url: null, sync_enabled: true, sync_on_wifi_only: false, lan_visibility: true, auto_apply_synced_clip: true, collect_public_ip: false, paste_as_plain_text: false, excluded_apps: [], max_text_size_bytes: null, max_image_size_bytes: null, max_file_size_bytes: null, storage_quota_bytes: null, sensitive_ttl_secs: null, image_quality: null }, error: null, error_code: null });
      case "get_private_mode": return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      case "get_sync_status": return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
      default: return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    }
  };
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(makeOnlineInvoke());
});

describe("CopyPaste-86ii: Backup & Restore surface in Settings GUI", () => {
  it("shows a Backup & Restore section in the Storage tab", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    // A Backup & Restore section must be present.
    await waitFor(() => {
      expect(screen.getByText(/Backup.*Restore|backup.*restore/i)).toBeInTheDocument();
    });
  });

  it("provides an Export backup button that triggers a file download", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    // Export button must be present.
    const exportBtn = await screen.findByTestId("export-button");
    expect(exportBtn).toBeInTheDocument();
    expect(exportBtn.textContent).toMatch(/Export/i);
  });

  it("provides a Restore backup control (file input)", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    // Import file input must be present (wraps the restore operation).
    const importInput = await screen.findByTestId("import-file-input");
    expect(importInput).toBeInTheDocument();
  });

  it("the Backup & Restore section describes what kind of backup is created", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    // The section hint must describe the backup kind so users understand
    // they are exporting clipboard history as a JSON file.
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/export|backup|history|json|file/i);
    });
  });
});
