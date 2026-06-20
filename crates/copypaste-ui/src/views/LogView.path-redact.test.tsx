/**
 * CopyPaste-2b3i — LogView subtitle must not leak the absolute /Users/<username> path.
 *
 * The log directory path from logDirPath() is an absolute macOS path like
 * /Users/alice/Library/Logs/CopyPaste. Rendering it verbatim leaks the
 * username to screen recordings, screenshots, and accessibility APIs.
 *
 * The fix: relativize the path to ~/Library/Logs/CopyPaste before display.
 * The full absolute path may be kept in the `title` attribute (tooltip) for
 * power users, but must not appear as visible text.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

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

beforeEach(() => {
  readLogs.mockReset().mockResolvedValue("INFO daemon started");
});

afterEach(() => {
  vi.useRealTimers();
});

describe("LogView — path redaction in subtitle (CopyPaste-2b3i)", () => {
  it("does not render the absolute /Users/<username> path as visible text", async () => {
    logDirPath
      .mockReset()
      .mockResolvedValue("/Users/alice/Library/Logs/CopyPaste");

    render(<LogView />);

    // Wait for loading to complete.
    await waitFor(() => {
      expect(screen.queryByText(/loading/i)).not.toBeInTheDocument();
    });

    // The absolute path must NOT appear as visible text in the subtitle.
    expect(document.body.textContent).not.toContain("/Users/alice");
    expect(document.body.textContent).not.toContain("/Users/");
  });

  it("renders the redacted ~/Library/Logs/CopyPaste in the subtitle", async () => {
    logDirPath
      .mockReset()
      .mockResolvedValue("/Users/bob/Library/Logs/CopyPaste");

    render(<LogView />);

    await waitFor(() => {
      // The ~/…  path must be the visible subtitle.
      expect(screen.queryByText("~/Library/Logs/CopyPaste")).toBeInTheDocument();
    });
  });

  it("relativizes any path that starts with /Users/ to ~/...", async () => {
    // Edge case: user with a different username still gets redacted.
    logDirPath
      .mockReset()
      .mockResolvedValue("/Users/testuser123/Library/Logs/CopyPaste");

    render(<LogView />);

    await waitFor(() => {
      expect(screen.queryByText(/loading/i)).not.toBeInTheDocument();
    });

    // The raw username must not appear as visible text.
    expect(document.body.textContent).not.toContain("/Users/testuser123");
    // The ~ form must appear.
    expect(document.body.textContent).toContain("~/Library/Logs/CopyPaste");
  });
});
