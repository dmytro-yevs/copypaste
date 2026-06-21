/**
 * ERR-1 regression: Popup must not leak socket paths or raw IPC transport
 * strings into the DOM on any error path.
 *
 * bd CopyPaste-bdac.87
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri runtime mocks — must be set up BEFORE importing Popup.
// ---------------------------------------------------------------------------

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onFocusChanged: vi.fn().mockResolvedValue(() => {}),
    hide: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: { getByLabel: vi.fn().mockResolvedValue(null) },
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build an IPC transport error that contains the full socket path. */
function socketPathError(user = "alice") {
  const socketPath = `/Users/${user}/.local/share/copypaste/copypaste.sock`;
  return { ok: false, data: null, error: `daemon_offline:${socketPath}`, error_code: "daemon_offline" };
}

function offlineReply() {
  return { ok: false, data: null, error: null, error_code: "daemon_offline" };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Popup ERR-1 — socket-path leak prevention (CopyPaste-bdac.87)", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("does not render the socket path when the daemon is offline (daemon_offline code)", async () => {
    const socketPath = "/Users/alice/.local/share/copypaste/copypaste.sock";
    invoke.mockResolvedValue(
      { ok: false, data: null, error: `daemon_offline:${socketPath}`, error_code: "daemon_offline" }
    );

    const { Popup } = await import("./Popup");
    render(<Popup />);

    await waitFor(() => {
      // The offline EmptyState must render.
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/background service|offline|not running/i);
    });

    // The raw socket path MUST NOT appear anywhere in the DOM.
    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/alice");
    expect(document.body.textContent).not.toContain("daemon_offline:");
  });

  it("does not render a raw IpcError message containing the socket path on unclassified error", async () => {
    const socketPath = "/Users/bob/.local/share/copypaste/copypaste.sock";
    // An unclassified error (not daemon_offline, not ipc_not_ready) whose
    // message embeds the full socket path — exactly the ERR-1 scenario.
    invoke.mockResolvedValue(
      { ok: false, data: null, error: `connect ENOENT ${socketPath}`, error_code: "some_unknown_code" }
    );

    const { Popup } = await import("./Popup");
    render(<Popup />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      // Some error UI must appear — the generic "Something went wrong" EmptyState.
      expect(body).toMatch(/something went wrong|could not be reached|try again/i);
    });

    // The raw socket path MUST NOT appear anywhere in the DOM.
    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/bob");
    expect(document.body.textContent).not.toContain("connect ENOENT");
    expect(document.body.textContent).not.toContain("ENOENT");
  });

  it("shows a friendly error for unclassified IPC errors (not raw message)", async () => {
    // Simulate any error that is NOT daemon_offline or ipc_not_ready.
    invoke.mockResolvedValue(
      { ok: false, data: null, error: "internal error at /some/path/daemon.sock", error_code: "internal_error" }
    );

    const { Popup } = await import("./Popup");
    render(<Popup />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      // Must show a generic friendly message, not the raw error string.
      expect(body).toMatch(/something went wrong|could not be reached|try again/i);
      expect(body).not.toContain("internal error at");
      expect(body).not.toContain("/some/path");
    });
  });

  it("renders 'Starting up…' for ipc_not_ready without leaking internals", async () => {
    invoke.mockResolvedValue(
      { ok: false, data: null, error: "ipc not ready at /Users/carol/.local/share/copypaste/daemon.sock", error_code: "ipc_not_ready" }
    );

    const { Popup } = await import("./Popup");
    render(<Popup />);

    await waitFor(() => {
      const body = document.body.textContent ?? "";
      expect(body).toMatch(/starting up|initialising|ready in a moment/i);
    });

    expect(document.body.textContent).not.toContain("/Users/carol");
    expect(document.body.textContent).not.toContain("ipc not ready at");
  });
});
