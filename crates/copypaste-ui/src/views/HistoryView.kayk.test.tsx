/**
 * CopyPaste-kayk — "Clear all" action added to HistoryView for parity with
 * Android and CLI. The action is gated behind a ConfirmModal.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
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

function makeEntry(id: string, wallTime = 1_700_000_000_000) {
  return {
    id,
    content_type: "text",
    preview: `Item ${id}`,
    is_sensitive: false,
    wall_time: wallTime,
    pinned: false,
  };
}

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

function setupOnlineWithItems(items: ReturnType<typeof makeEntry>[]) {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "history_page") {
      return Promise.resolve(ipcOk({ items, total: items.length, own_device_id: "dev-1" }));
    }
    if (args?.method === "get_private_mode") {
      return Promise.resolve(ipcOk({ private_mode: false }));
    }
    if (args?.method === "status") {
      return Promise.resolve(ipcOk({ status: "running", ready: true, degraded: false }));
    }
    if (args?.method === "delete_all") {
      return Promise.resolve(ipcOk({ deleted: items.length }));
    }
    if (args?.method === "search") {
      return Promise.resolve(ipcOk({ items: [] }));
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
// CopyPaste-kayk: "Clear all" history action
// ---------------------------------------------------------------------------

describe("CopyPaste-kayk: Clear all history action in HistoryView", () => {
  it("shows a Clear all button in the toolbar when items exist", async () => {
    setupOnlineWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const clearBtn = screen.getByRole("button", { name: /clear all/i });
    expect(clearBtn).toBeInTheDocument();
  });

  it("clicking Clear all opens a ConfirmModal dialog", async () => {
    setupOnlineWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const clearBtn = screen.getByRole("button", { name: /clear all/i });
    fireEvent.click(clearBtn);

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();
    expect(dialog.textContent).toMatch(/clear|delete|history/i);
  });

  it("does NOT call delete_all when user cancels the Clear all modal", async () => {
    setupOnlineWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const clearBtn = screen.getByRole("button", { name: /clear all/i });
    fireEvent.click(clearBtn);

    await screen.findByRole("dialog");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    fireEvent.click(cancelBtn);

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const deleteAllCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "delete_all",
    );
    expect(deleteAllCalls).toHaveLength(0);
  });

  it("calls delete_all after user confirms Clear all", async () => {
    const items = [makeEntry("a"), makeEntry("b")];
    let itemStore = items.slice();
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: itemStore, total: itemStore.length, own_device_id: "dev-1" }));
      }
      if (args?.method === "get_private_mode") {
        return Promise.resolve(ipcOk({ private_mode: false }));
      }
      if (args?.method === "status") {
        return Promise.resolve(ipcOk({ status: "running", ready: true, degraded: false }));
      }
      if (args?.method === "delete_all") {
        itemStore = [];
        return Promise.resolve(ipcOk({ deleted: 2 }));
      }
      if (args?.method === "search") {
        return Promise.resolve(ipcOk({ items: [] }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const clearBtn = screen.getByRole("button", { name: /clear all/i });
    fireEvent.click(clearBtn);

    await screen.findByRole("dialog");
    const confirmBtn = screen.getByTestId("confirm-modal-confirm-btn");

    await act(async () => {
      fireEvent.click(confirmBtn);
    });

    await waitFor(() => {
      const deleteAllCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "delete_all",
      );
      expect(deleteAllCalls.length).toBeGreaterThan(0);
    });
  });
});
