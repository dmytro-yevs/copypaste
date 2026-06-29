/**
 * Tests for the 5 UX fixes:
 *
 * CopyPaste-fjvz — mass-delete requires a confirmation modal.
 * CopyPaste-xhns — empty state shows private-mode messaging when active.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mock — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
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

function setupInvokeWithItems(
  items: ReturnType<typeof makeEntry>[],
  privateMode = false,
) {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "history_page") {
      return Promise.resolve(ipcOk({ items, total: items.length }));
    }
    if (args?.method === "get_private_mode") {
      return Promise.resolve(ipcOk({ private_mode: privateMode }));
    }
    if (args?.method === "status") {
      return Promise.resolve(
        ipcOk({ status: "running", private_mode: privateMode, ready: true, degraded: false }),
      );
    }
    if (args?.method === "delete_item") {
      return Promise.resolve(ipcOk({}));
    }
    return Promise.reject("daemon_offline:/tmp/x.sock");
  });
}

// ---------------------------------------------------------------------------
// CopyPaste-fjvz: bulk delete must show a confirmation modal
// ---------------------------------------------------------------------------

describe("CopyPaste-fjvz: bulk delete requires confirmation modal", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("shows a confirmation modal when Delete is clicked in the bulk bar", async () => {
    setupInvokeWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    // Wait for items to render.
    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    // Click the checkbox to enter multi-select mode.
    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);

    // The bulk action bar should be visible.
    const deleteBtn = await screen.findByRole("button", { name: /^delete$/i });
    fireEvent.click(deleteBtn);

    // A confirmation dialog must appear instead of deleting immediately.
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();
    // The dialog must warn about the destructive action.
    expect(dialog.textContent).toMatch(/delete|remove|confirm/i);
  });

  it("does NOT call delete_item if the user cancels the confirmation modal", async () => {
    setupInvokeWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);

    const deleteBtn = await screen.findByRole("button", { name: /^delete$/i });
    fireEvent.click(deleteBtn);

    // Dialog appears — cancel it.
    await screen.findByRole("dialog");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    fireEvent.click(cancelBtn);

    // Dialog must close.
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    // delete_item must NOT have been called.
    const deleteCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "delete_item",
    );
    expect(deleteCalls).toHaveLength(0);
  });

  it("calls delete_item after the user confirms in the modal", async () => {
    setupInvokeWithItems([makeEntry("a"), makeEntry("b")]);

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item a")).toBeInTheDocument());

    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);

    const deleteBtn = await screen.findByRole("button", { name: /^delete$/i });
    fireEvent.click(deleteBtn);

    // Dialog appears — confirm.
    await screen.findByRole("dialog");
    const confirmBtn = screen.getByTestId("confirm-modal-confirm-btn");

    await act(async () => {
      fireEvent.click(confirmBtn);
    });

    // delete_item must be called for the selected item.
    await waitFor(() => {
      const deleteCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "delete_item",
      );
      expect(deleteCalls.length).toBeGreaterThan(0);
    });
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-xhns: private mode empty state
// ---------------------------------------------------------------------------

describe("CopyPaste-xhns: private-mode empty state messaging", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("shows private-mode messaging when private mode is active and history is empty", async () => {
    // Private mode active, no items.
    setupInvokeWithItems([], /* privateMode= */ true);

    render(<HistoryView />);

    // Wait for load to complete.
    await waitFor(() => {
      // Should NOT show the default "Copy something" message.
      expect(screen.queryByText(/copy something/i)).not.toBeInTheDocument();
    });

    // Should show private-mode specific messaging.
    const text = document.body.textContent ?? "";
    expect(text).toMatch(/private mode/i);
  });

  it("shows default empty-state when private mode is off and history is empty", async () => {
    // Private mode off, no items.
    setupInvokeWithItems([], /* privateMode= */ false);

    render(<HistoryView />);

    await waitFor(() => {
      // Default "Copy something" text should be visible.
      expect(screen.getByText(/copy something/i)).toBeInTheDocument();
    });
  });
});
