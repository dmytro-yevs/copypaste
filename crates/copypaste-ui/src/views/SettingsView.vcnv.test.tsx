/**
 * CopyPaste-vcnv: Restore backup must require a ConfirmModal before replacing
 * the live database, to prevent accidental data loss.
 *
 * Strategy: navigate to Storage tab, simulate a file selection, and assert
 * that a confirmation dialog appears BEFORE any import IPC call is made.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

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
// Daemon stub
// ---------------------------------------------------------------------------

function makeOnlineInvoke() {
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
      default:
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    }
  };
}

const BACKUP_PAYLOAD = JSON.stringify({
  items: [
    { content_type: "text", content_bytes_b64: btoa("hello"), created_at_ms: Date.now(), metadata: null },
  ],
});

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(makeOnlineInvoke());
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Helper: navigate to Storage tab
// ---------------------------------------------------------------------------

async function renderAndGoToStorage() {
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CopyPaste-vcnv: Restore backup requires ConfirmModal before import", () => {
  it("shows a confirmation dialog after selecting a file, before calling import IPC", async () => {
    await renderAndGoToStorage();

    const fileInput = screen.getByTestId("import-file-input") as HTMLInputElement;

    // Stub FileReader to return a valid backup JSON synchronously.
    vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (
      this: FileReader,
    ) {
      Object.defineProperty(this, "result", { value: BACKUP_PAYLOAD, writable: true });
      this.onload?.({ target: this } as ProgressEvent<FileReader>);
    });

    const file = new File([BACKUP_PAYLOAD], "backup.json", { type: "application/json" });

    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
    });

    // A confirmation dialog must appear BEFORE the import is executed.
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();

    // The import IPC method must NOT have been called yet.
    const importCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "import",
    );
    expect(importCalls).toHaveLength(0);
  });

  it("cancelling the confirmation dialog does NOT call the import IPC method", async () => {
    await renderAndGoToStorage();

    const fileInput = screen.getByTestId("import-file-input") as HTMLInputElement;

    vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (
      this: FileReader,
    ) {
      Object.defineProperty(this, "result", { value: BACKUP_PAYLOAD, writable: true });
      this.onload?.({ target: this } as ProgressEvent<FileReader>);
    });

    const file = new File([BACKUP_PAYLOAD], "backup.json", { type: "application/json" });

    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
    });

    // Wait for the dialog to appear.
    await screen.findByRole("dialog");

    // Cancel the confirmation.
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    await act(async () => {
      fireEvent.click(cancelBtn);
    });

    // Dialog closes.
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    // Import must NOT have been called.
    const importCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "import",
    );
    expect(importCalls).toHaveLength(0);
  });

  it("confirming the dialog proceeds to call the import IPC method", async () => {
    invoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "app_version") return Promise.resolve("0.7.5");
      if (cmd === "check_accessibility_permission") return Promise.resolve(true);
      const method = (args as { method?: string } | undefined)?.method;
      if (method === "status") return Promise.resolve({ ok: true, data: { ready: true, degraded: false, build_version: "0.7.5" }, error: null, error_code: null });
      if (method === "get_config") return Promise.resolve({ ok: true, data: { p2p_enabled: true, supabase_url: null, supabase_anon_key: null, relay_url: null, sync_enabled: true, sync_on_wifi_only: false, lan_visibility: true, auto_apply_synced_clip: true, collect_public_ip: false, paste_as_plain_text: false, excluded_apps: [], max_text_size_bytes: null, max_image_size_bytes: null, max_file_size_bytes: null, storage_quota_bytes: null, sensitive_ttl_secs: null, image_quality: null }, error: null, error_code: null });
      if (method === "get_private_mode") return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
      if (method === "get_sync_status") return Promise.resolve({ ok: true, data: { last_sync_ms: null, supabase_url: null }, error: null, error_code: null });
      if (method === "import") return Promise.resolve({ ok: true, data: { inserted: 1, skipped: 0 }, error: null, error_code: null });
      return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
    });

    await renderAndGoToStorage();

    const fileInput = screen.getByTestId("import-file-input") as HTMLInputElement;

    vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (
      this: FileReader,
    ) {
      Object.defineProperty(this, "result", { value: BACKUP_PAYLOAD, writable: true });
      this.onload?.({ target: this } as ProgressEvent<FileReader>);
    });

    const file = new File([BACKUP_PAYLOAD], "backup.json", { type: "application/json" });

    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
    });

    // Wait for dialog.
    await screen.findByRole("dialog");

    // Confirm the restore.
    const confirmBtn = screen.getByRole("button", { name: /restore|import|confirm|ok|yes/i });
    await act(async () => {
      fireEvent.click(confirmBtn);
    });

    // Import must now be called.
    await waitFor(() => {
      const importCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "import",
      );
      expect(importCalls.length).toBeGreaterThan(0);
    });
  });
});
