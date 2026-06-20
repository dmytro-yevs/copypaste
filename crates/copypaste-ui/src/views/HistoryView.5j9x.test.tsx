/**
 * CopyPaste-5j9x — "Reset database" in the degraded error state must use the
 * shared ConfirmModal, not misclick-prone inline Yes/No buttons.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mocks — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { HistoryView } from "./HistoryView";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

function setupDegradedDaemon() {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "history_page") {
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
      return Promise.reject({ code: "ipc_not_ready", message: "db not ready" });
    }
    if (args?.method === "status") {
      return Promise.resolve(
        ipcOk({ status: "degraded", private_mode: false, ready: false, degraded: true, degraded_reason: "key mismatch" }),
      );
    }
    if (args?.method === "get_private_mode") {
      return Promise.resolve(ipcOk({ private_mode: false }));
    }
    if (args?.method === "reset_database") {
      return Promise.resolve(ipcOk({ ok: true }));
    }
    return Promise.reject("daemon_offline:/tmp/x.sock");
  });
}

// ---------------------------------------------------------------------------
// Setup / teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  invoke.mockReset();
  useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// CopyPaste-5j9x: Reset database must use ConfirmModal (not inline buttons)
// ---------------------------------------------------------------------------

describe("CopyPaste-5j9x: Reset database uses ConfirmModal in degraded state", () => {
  it("shows a role=dialog modal when Reset database button is clicked", async () => {
    setupDegradedDaemon();

    render(<HistoryView />);

    // Wait for the degraded error state to render.
    const resetBtn = await screen.findByRole("button", { name: /reset database/i });
    fireEvent.click(resetBtn);

    // A modal dialog must appear (not just inline Yes/No text).
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();
    expect(dialog.textContent).toMatch(/erase|reset|database/i);
  });

  it("does NOT show inline 'Erase and reset?' text before clicking Reset database", async () => {
    setupDegradedDaemon();

    render(<HistoryView />);

    await screen.findByRole("button", { name: /reset database/i });

    // Inline text "Erase and reset?" must NOT be visible at all.
    expect(screen.queryByText(/erase and reset\?/i)).not.toBeInTheDocument();
  });

  it("closes modal without calling reset_database when user cancels", async () => {
    setupDegradedDaemon();

    render(<HistoryView />);

    const resetBtn = await screen.findByRole("button", { name: /reset database/i });
    fireEvent.click(resetBtn);

    await screen.findByRole("dialog");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    fireEvent.click(cancelBtn);

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const resetCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "reset_database",
    );
    expect(resetCalls).toHaveLength(0);
  });
});
