/**
 * ERR-2 regression: HistoryView must not leak socket paths or raw IPC transport
 * strings into the DOM on any error path (load failure, reset, clear-all).
 *
 * bd CopyPaste-bdac.90
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri runtime mocks — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: vi.fn().mockReturnValue({
    onDragDropEvent: vi.fn().mockResolvedValue(() => {}),
  }),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

function ipcErr(message: string, code: string | null = null) {
  return { ok: false, data: null, error: message, error_code: code };
}

// A minimal status reply used when the error path checks daemon status.
const STATUS_OK = ipcOk({ status: "running", private_mode: false, ready: true, degraded: false });

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("HistoryView ERR-2 — socket-path leak prevention (CopyPaste-bdac.90)", () => {
  beforeEach(() => {
    invoke.mockReset();
    // Default: status is always OK (so we can test other error codes in isolation).
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "status") return Promise.resolve(STATUS_OK);
      if (args?.method === "get_private_mode") return Promise.resolve(ipcOk({ private_mode: false }));
      return Promise.resolve(ipcErr("daemon_offline", "daemon_offline"));
    });
  });

  it("does not render the socket path when history_page fails with a transport error", async () => {
    const socketPath = "/Users/alice/.local/share/copypaste/copypaste.sock";

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "status") return Promise.resolve(STATUS_OK);
      if (args?.method === "get_private_mode") return Promise.resolve(ipcOk({ private_mode: false }));
      if (args?.method === "history_page") {
        return Promise.resolve(
          ipcErr(`connect ENOENT ${socketPath}`, "some_internal_error")
        );
      }
      return Promise.resolve(ipcErr("daemon_offline", "daemon_offline"));
    });

    const { HistoryView } = await import("./HistoryView");
    render(<HistoryView />);

    // Wait until an error state renders (not the loading spinner).
    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).not.toContain("Loading");
    }, { timeout: 3000 });

    // The raw socket path and internal strings MUST NOT appear in the DOM.
    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/alice");
    expect(document.body.textContent).not.toContain("connect ENOENT");
    expect(document.body.textContent).not.toContain("ENOENT");
  });

  it("does not render a raw IpcError message containing the socket path", async () => {
    const socketPath = "/Users/bob/.local/share/copypaste/daemon.sock";

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "status") return Promise.resolve(STATUS_OK);
      if (args?.method === "get_private_mode") return Promise.resolve(ipcOk({ private_mode: false }));
      if (args?.method === "history_page") {
        return Promise.resolve(
          ipcErr(`open ${socketPath}: no such file or directory`, "storage_error")
        );
      }
      return Promise.resolve(ipcErr("daemon_offline", "daemon_offline"));
    });

    const { HistoryView } = await import("./HistoryView");
    render(<HistoryView />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).not.toContain("Loading");
    }, { timeout: 3000 });

    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/bob");
    expect(document.body.textContent).not.toContain("no such file or directory");
  });

  it("shows a friendly error detail (not the raw error) in the error state", async () => {
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "status") return Promise.resolve(STATUS_OK);
      if (args?.method === "get_private_mode") return Promise.resolve(ipcOk({ private_mode: false }));
      if (args?.method === "history_page") {
        return Promise.resolve(
          ipcErr("internal: open /var/run/copypaste.sock: permission denied", "permission_denied")
        );
      }
      return Promise.resolve(ipcErr("daemon_offline", "daemon_offline"));
    });

    const { HistoryView } = await import("./HistoryView");
    render(<HistoryView />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      // Must show a friendly message — the friendly copy for permission_denied.
      expect(body).toMatch(/permission denied|something went wrong|failed to load/i);
    });

    // MUST NOT render the raw path from the error message.
    expect(document.body.textContent).not.toContain("/var/run/copypaste.sock");
    expect(document.body.textContent).not.toContain("internal: open");
  });

  it("does not render username from a daemon_offline transport error in String() form", async () => {
    // Previously `String(err)` would produce "daemon_offline:/Users/carol/…"
    // which contains the username. Verify that the DOM never contains /Users/.
    const socketPath = "/Users/carol/.local/share/copypaste/copypaste.sock";

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "status") return Promise.resolve(STATUS_OK);
      if (args?.method === "get_private_mode") return Promise.resolve(ipcOk({ private_mode: false }));
      if (args?.method === "history_page") {
        // Simulate a raw transport rejection string (not an IpcError).
        return Promise.reject(`daemon_offline:${socketPath}`);
      }
      return Promise.resolve(ipcErr("daemon_offline", "daemon_offline"));
    });

    const { HistoryView } = await import("./HistoryView");
    render(<HistoryView />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).not.toContain("Loading");
    }, { timeout: 3000 });

    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/carol");
    expect(document.body.textContent).not.toContain("daemon_offline:");
  });
});
