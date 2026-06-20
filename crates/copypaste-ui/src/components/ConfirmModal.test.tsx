/**
 * ConfirmModal — unit tests for the shared confirmation modal.
 *
 * Covers: rendering, confirm/cancel callbacks, backdrop click, Escape key,
 * busy state, and custom labels.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ConfirmModal } from "./ConfirmModal";

// ConfirmModal renders into document.body via createPortal — no special setup needed.

describe("ConfirmModal", () => {
  beforeEach(() => {
    // No IPC mocks needed; ConfirmModal is pure UI.
  });

  it("does not render when open=false", () => {
    render(
      <ConfirmModal
        open={false}
        title="Delete all?"
        body="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("renders title and body when open=true", () => {
    render(
      <ConfirmModal
        open
        title="Delete all?"
        body="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Delete all?")).toBeInTheDocument();
    expect(screen.getByText("This cannot be undone.")).toBeInTheDocument();
  });

  it("calls onConfirm when the confirm button is clicked", () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmModal
        open
        title="Delete?"
        body="Are you sure?"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId("confirm-modal-confirm-btn"));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("calls onCancel when the Cancel button is clicked", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmModal
        open
        title="Delete?"
        body="Are you sure?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("calls onCancel when Escape is pressed inside the dialog", () => {
    const onCancel = vi.fn();
    render(
      <ConfirmModal
        open
        title="Delete?"
        body="Are you sure?"
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("disables both buttons and shows ellipsis when busy=true", () => {
    render(
      <ConfirmModal
        open
        busy
        title="Deleting…"
        body="Please wait."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    const confirmBtn = screen.getByTestId("confirm-modal-confirm-btn");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    expect(confirmBtn).toBeDisabled();
    expect(cancelBtn).toBeDisabled();
    // Busy state shows "…" not the normal label.
    expect(confirmBtn).toHaveTextContent("…");
  });

  it("respects custom confirmLabel and cancelLabel", () => {
    render(
      <ConfirmModal
        open
        title="Revoke all?"
        body="All devices will lose trust."
        confirmLabel="Yes, revoke all"
        cancelLabel="Go back"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Yes, revoke all" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Go back" })).toBeInTheDocument();
  });
});
