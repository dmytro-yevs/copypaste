/**
 * Tests for the 85n9 Backup & Restore panel in SettingsView (Storage tab).
 *
 * Strategy:
 *  - Mock @tauri-apps/api/core `invoke` to simulate a healthy daemon.
 *  - Navigate to the Storage tab and interact with the Export button and the
 *    file-input Import control.
 *  - Assert: export calls `ipcCall("export", …)` and the result triggers a
 *    Blob download anchor; import reads a file via FileReader and calls
 *    `ipcCall("import", …)`.
 *
 * No fs or dialog Tauri plugin capability is exercised — the test confirms that
 * the entire flow works in-browser (Blob + anchor for export, FileReader for
 * import) without any native plugin.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock the Tauri core bridge BEFORE importing any module that pulls in ipc.ts.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Browser API stubs
// ---------------------------------------------------------------------------

// URL.createObjectURL / revokeObjectURL are not implemented in jsdom.
const createdUrls: string[] = [];
const revokedUrls: string[] = [];
const anchorClicks: HTMLAnchorElement[] = [];

beforeEach(() => {
  invoke.mockReset();
  createdUrls.length = 0;
  revokedUrls.length = 0;
  anchorClicks.length = 0;

  // Stub URL object methods.
  vi.stubGlobal("URL", {
    ...URL,
    createObjectURL: vi.fn((blob: Blob) => {
      const url = `blob:mock-${createdUrls.length}`;
      createdUrls.push(url);
      // Store blob for later inspection if needed
      void blob;
      return url;
    }),
    revokeObjectURL: vi.fn((url: string) => {
      revokedUrls.push(url);
    }),
  });

  // Intercept anchor clicks to capture download attempts (jsdom doesn't navigate).
  const origAppendChild = document.body.appendChild.bind(document.body);
  vi.spyOn(document.body, "appendChild").mockImplementation((node) => {
    if (node instanceof HTMLAnchorElement) {
      anchorClicks.push(node);
      // Don't actually click — jsdom navigation is a no-op and creates noise.
    }
    return origAppendChild(node);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// Daemon fixture helpers
// ---------------------------------------------------------------------------

/** Minimal item fixture matching the export shape from mockIpc.ts. */
const FIXTURE_EXPORT_DATA = {
  items: [
    {
      content_type: "text",
      content_bytes_b64: btoa("hello world"),
      created_at_ms: Date.now() - 1000,
      metadata: null,
    },
    {
      content_type: "text",
      content_bytes_b64: btoa("second item"),
      created_at_ms: Date.now() - 2000,
      metadata: null,
    },
  ],
};

function makeOnlineInvoke(
  overrides: Record<string, (args?: unknown) => unknown> = {},
) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (overrides[cmd]) return Promise.resolve(overrides[cmd](args));

    if (cmd === "ipc_call") {
      const method = (args as { method?: string } | undefined)?.method;
      switch (method) {
        case "status":
          return Promise.resolve({
            ok: true,
            data: { status: "running", ready: true, degraded: false, degraded_reason: null, build_version: "0.7.1" },
            error: null,
            error_code: null,
          });
        case "get_config":
          return Promise.resolve({
            ok: true,
            data: { p2p_enabled: true, supabase_url: null, supabase_anon_key: null, max_text_size_bytes: 10 * 1024 * 1024, max_image_size_bytes: 25 * 1024 * 1024, max_file_size_bytes: 100 * 1024 * 1024, storage_quota_bytes: 10 * 1024 * 1024 * 1024, sensitive_ttl_secs: 30, image_quality: 100, sync_on_wifi_only: false, sound_on_copy: false, notify_on_copy: false },
            error: null,
            error_code: null,
          });
        case "get_private_mode":
          return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
        case "get_sync_status":
          return Promise.resolve({ ok: true, data: { passphrase_set: false, supabase_configured: false, signed_in: false, email: null, last_sync_ms: null }, error: null, error_code: null });
        case "export":
          return Promise.resolve({ ok: true, data: FIXTURE_EXPORT_DATA, error: null, error_code: null });
        case "import":
          return Promise.resolve({
            ok: true,
            data: { inserted: ((args as { params?: { items?: unknown[] } })?.params?.items ?? []).length, skipped: 0 },
            error: null,
            error_code: null,
          });
        default:
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
    }

    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.7.1");
    return Promise.resolve(undefined);
  };
}

// ---------------------------------------------------------------------------
// Helper: navigate SettingsView to the Storage tab
// ---------------------------------------------------------------------------

async function renderAndGoToStorage() {
  invoke.mockImplementation(makeOnlineInvoke());
  render(
    <ErrorBoundary label="Settings">
      <SettingsView />
    </ErrorBoundary>,
  );

  // Wait for "ready" state (offline banner disappears).
  await waitFor(() => {
    expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
  });

  // Click the Storage tab.
  const storageTab = await screen.findByText("Storage");
  await act(async () => {
    fireEvent.click(storageTab);
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("85n9 Backup & Restore panel", () => {
  it("renders the Backup & Restore section with Export and Import controls in the Storage tab", async () => {
    await renderAndGoToStorage();

    // Section header present.
    expect(screen.getByText(/Backup & Restore/i)).toBeInTheDocument();

    // Export row.
    expect(screen.getByText(/Export backup/i)).toBeInTheDocument();
    expect(screen.getByTestId("export-button")).toBeInTheDocument();

    // Import row — label renamed to "Import history" (bdac.73).
    expect(screen.getByText(/Import history/i)).toBeInTheDocument();
    expect(screen.getByTestId("import-file-input")).toBeInTheDocument();

    // Include-sensitive checkbox defaults to unchecked.
    const sensitiveCheckbox = screen.getByRole("checkbox", { name: /include sensitive/i });
    expect(sensitiveCheckbox).not.toBeChecked();
  });

  it("export button calls ipcCall('export') and triggers a Blob download anchor", async () => {
    await renderAndGoToStorage();

    const exportBtn = screen.getByTestId("export-button");
    await act(async () => {
      fireEvent.click(exportBtn);
    });

    // The daemon export method must have been called.
    await waitFor(() => {
      const exportCalls = invoke.mock.calls.filter(
        ([cmd, args]: [string, unknown]) =>
          cmd === "ipc_call" && (args as { method?: string })?.method === "export",
      );
      expect(exportCalls.length).toBeGreaterThanOrEqual(1);
    });

    // A Blob URL was created.
    await waitFor(() => {
      expect(createdUrls.length).toBeGreaterThanOrEqual(1);
    });

    // A download anchor was appended.
    await waitFor(() => {
      const downloadAnchors = anchorClicks.filter(
        (a) => a.download && a.download.startsWith("copypaste-backup-"),
      );
      expect(downloadAnchors.length).toBeGreaterThanOrEqual(1);
    });

    // Success message shown.
    await waitFor(() => {
      expect(screen.getByText(/exported \d+ items?/i)).toBeInTheDocument();
    });
  });

  it("export passes include_sensitive=true when the checkbox is checked", async () => {
    await renderAndGoToStorage();

    // Check the "Include sensitive items" checkbox.
    const sensitiveCheckbox = screen.getByRole("checkbox", { name: /include sensitive/i });
    await act(async () => {
      fireEvent.click(sensitiveCheckbox);
    });

    // Warning text should appear.
    expect(screen.getByText(/sensitive items will be exported as plaintext/i)).toBeInTheDocument();

    // Click export.
    const exportBtn = screen.getByTestId("export-button");
    await act(async () => {
      fireEvent.click(exportBtn);
    });

    await waitFor(() => {
      const exportCalls = invoke.mock.calls.filter(
        ([cmd, args]: [string, unknown]) =>
          cmd === "ipc_call" && (args as { method?: string })?.method === "export",
      );
      expect(exportCalls.length).toBeGreaterThanOrEqual(1);
      // The params must include include_sensitive: true.
      const lastCall = exportCalls[exportCalls.length - 1];
      const params = (lastCall[1] as { params?: { include_sensitive?: boolean } })?.params;
      expect(params?.include_sensitive).toBe(true);
    });
  });

  it("import file input calls ipcCall('import') with parsed items and shows result count", async () => {
    await renderAndGoToStorage();

    // Create a minimal backup JSON blob matching the export format.
    const backupPayload = JSON.stringify(FIXTURE_EXPORT_DATA);
    const file = new File([backupPayload], "copypaste-backup-2026-06-19.json", {
      type: "application/json",
    });

    const fileInput = screen.getByTestId("import-file-input") as HTMLInputElement;

    // Simulate FileReader returning the backup JSON. jsdom's FileReader doesn't
    // call onload automatically for File objects, so we stub readAsText.
    vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (
      this: FileReader,
      _blob: Blob,
    ) {
      // Trigger onload synchronously so the test doesn't need fake timers.
      Object.defineProperty(this, "result", { value: backupPayload, writable: true });
      this.onload?.({ target: this } as ProgressEvent<FileReader>);
    });

    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
    });

    // vcnv: a confirmation modal appears before the import — click "Import" to proceed.
    // Confirm button renamed from "Restore" to "Import" (bdac.73).
    const restoreBtn = await screen.findByRole("button", { name: /import/i });
    await act(async () => {
      fireEvent.click(restoreBtn);
    });

    // The daemon import method must have been called with the items array.
    await waitFor(() => {
      const importCalls = invoke.mock.calls.filter(
        ([cmd, args]: [string, unknown]) =>
          cmd === "ipc_call" && (args as { method?: string })?.method === "import",
      );
      expect(importCalls.length).toBeGreaterThanOrEqual(1);
      const importCall = importCalls[0];
      const params = (importCall[1] as { params?: { items?: unknown[] } })?.params;
      expect(Array.isArray(params?.items)).toBe(true);
      expect((params?.items ?? []).length).toBe(FIXTURE_EXPORT_DATA.items.length);
    });

    // Result message shown.
    await waitFor(() => {
      expect(screen.getByText(/imported \d+ items?/i)).toBeInTheDocument();
    });
  });

  it("shows an error when import JSON is invalid", async () => {
    await renderAndGoToStorage();

    const file = new File(["not-valid-json{{{"], "bad.json", { type: "application/json" });
    const fileInput = screen.getByTestId("import-file-input") as HTMLInputElement;

    vi.spyOn(FileReader.prototype, "readAsText").mockImplementation(function (
      this: FileReader,
      _blob: Blob,
    ) {
      Object.defineProperty(this, "result", { value: "not-valid-json{{{", writable: true });
      this.onload?.({ target: this } as ProgressEvent<FileReader>);
    });

    await act(async () => {
      fireEvent.change(fileInput, { target: { files: [file] } });
    });

    await waitFor(() => {
      expect(screen.getByText(/Import failed/i)).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// ro0r: migration_in_progress retry in ipcCall
// ---------------------------------------------------------------------------

describe("ro0r: migration_in_progress backoff retry in ipcCall", () => {
  it("retries on migration_in_progress and eventually succeeds", async () => {
    // First two calls return migration_in_progress; third succeeds.
    let callCount = 0;
    invoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "ipc_call") {
        const method = (args as { method?: string } | undefined)?.method;
        if (method === "status") {
          callCount++;
          if (callCount <= 2) {
            return Promise.resolve({
              ok: false,
              data: null,
              error: "migration in progress",
              error_code: "migration_in_progress",
            });
          }
          return Promise.resolve({
            ok: true,
            data: { status: "running", ready: true, degraded: false, degraded_reason: null },
            error: null,
            error_code: null,
          });
        }
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
      if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "app_version") return Promise.resolve("0.7.1");
      return Promise.resolve(undefined);
    });

    // Import ipcCall directly to test the retry logic in isolation.
    const { ipcCall } = await import("../lib/ipc");

    // Use fake timers to avoid real delays.
    vi.useFakeTimers();
    const resultPromise = ipcCall("status");

    // Advance timers to exhaust the backoff delays.
    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const result = await resultPromise;
    expect(result).toBeDefined();
    // Three total calls: 2 retried + 1 success.
    expect(callCount).toBe(3);

    vi.useRealTimers();
  });

  it("propagates error after exhausting MAX_MIGRATION_RETRIES", async () => {
    // Always return migration_in_progress to exhaust retries.
    invoke.mockResolvedValue({
      ok: false,
      data: null,
      error: "migration in progress",
      error_code: "migration_in_progress",
    });

    const { ipcCall, IpcError } = await import("../lib/ipc");

    vi.useFakeTimers();
    const resultPromise = ipcCall("status").catch((e) => e);

    await act(async () => {
      await vi.runAllTimersAsync();
    });

    const err = await resultPromise;
    expect(err).toBeInstanceOf(IpcError);
    expect((err as InstanceType<typeof IpcError>).code).toBe("migration_in_progress");

    vi.useRealTimers();
  });
});
