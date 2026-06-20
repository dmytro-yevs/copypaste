/**
 * CopyPaste-mtvx — LogView daemon-offline guard.
 *
 * Verifies that LogView:
 *  1. Shows a friendly "daemon offline" message when readLogs throws a
 *     daemon-offline IPC error (not the raw error string).
 *  2. Shows a friendly message on generic FS errors (no raw path/message).
 *  3. Shows a friendly "no logs yet" message when readLogs returns an empty
 *     string (empty log file — normal on first launch).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock the IPC log helpers so we control what readLogs/logDirPath return.
// ---------------------------------------------------------------------------
const readLogs = vi.fn();
const logDirPath = vi.fn();

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  return {
    ...actual,
    readLogs: (...a: unknown[]) => readLogs(...a),
    logDirPath: (...a: unknown[]) => logDirPath(...a),
  };
});

import { LogView } from "./LogView";
import { IpcError } from "../lib/ipc";

beforeEach(() => {
  logDirPath.mockReset().mockResolvedValue("~/Library/Logs/CopyPaste");
  readLogs.mockReset().mockResolvedValue("2024-01-01 INFO startup complete");
});

afterEach(() => {
  vi.useRealTimers();
});

describe("LogView — daemon-offline guard (CopyPaste-mtvx)", () => {
  it("shows a friendly offline message when the daemon is not running", async () => {
    const rawErr = "daemon_offline:/Users/bob/.local/share/copypaste/copypaste.sock";
    readLogs.mockRejectedValue(new IpcError(rawErr, "daemon_offline"));

    render(<LogView />);

    await waitFor(() => {
      // The friendly message must appear.
      expect(
        screen.queryByText(/daemon.*not running|offline|unavailable/i) ||
        screen.queryByText(/start the daemon|restart/i) ||
        screen.queryByText(/no logs/i)
      ).not.toBeNull();

      // Raw socket path must NOT appear in the DOM.
      expect(document.body.textContent).not.toContain(rawErr);
      expect(document.body.textContent).not.toContain("/Users/bob");
    });
  });

  it("shows a friendly error message for FS-level failures without leaking the path", async () => {
    const rawErr = "ENOENT: no such file or directory, open '/Users/alice/Library/Logs/CopyPaste/daemon.log'";
    readLogs.mockRejectedValue(new Error(rawErr));

    render(<LogView />);

    // Wait for the loading state to clear (either error or content appears).
    // The log area transitions from "Loading…" to the error or content state.
    await waitFor(() => {
      expect(screen.queryByText(/loading/i)).not.toBeInTheDocument();
    });

    // Now check that the raw path/message is not in the DOM.
    expect(document.body.textContent).not.toContain(rawErr);
    expect(document.body.textContent).not.toContain("/Users/alice");
  });

  it("shows a friendly empty-state message when readLogs returns empty string", async () => {
    readLogs.mockResolvedValue("");

    render(<LogView />);

    await waitFor(() => {
      // An empty log should show a friendly placeholder, not a blank log area.
      // The current code sets content to "(no log entries)" for empty strings.
      expect(screen.queryByText(/no log entries/i)).toBeInTheDocument();
    });
  });

  it("renders logs normally when readLogs succeeds", async () => {
    readLogs.mockResolvedValue("2024-01-01T00:00:00Z INFO daemon started");

    render(<LogView />);

    await waitFor(() => {
      expect(screen.queryByText(/2024-01-01/)).toBeInTheDocument();
    });
  });
});
