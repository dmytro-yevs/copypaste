import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AlertDialog } from "./alert-dialog";
import { Dialog } from "./dialog";
import { TooltipProvider } from "./tooltip";

describe("composed dialogs", () => {
  it("renders a controlled dialog and forwards focus handlers to its content", () => {
    const onOpenAutoFocus = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <TooltipProvider>
        <Dialog
          open
          onOpenChange={onOpenChange}
          title="Rename device"
          description="Choose a name"
          contentProps={{ onOpenAutoFocus }}
          footer={<button type="button">Save</button>}
        >
          <input aria-label="Device name" />
        </Dialog>
      </TooltipProvider>,
    );

    expect(screen.getByRole("dialog", { name: "Rename device" })).toBeTruthy();
    expect(screen.getByText("Choose a name")).toBeTruthy();
    expect(onOpenAutoFocus).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("keeps a confirmation open for an async owner and preserves a safe cancel", () => {
    const onClick = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <AlertDialog
        defaultOpen
        onOpenChange={onOpenChange}
        title="Delete history"
        description="This cannot be undone"
        cancel={{ label: "Cancel" }}
        action={{ label: "Delete", variant: "danger", onClick }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByRole("alertdialog", { name: "Delete history" })).toBeTruthy();
    expect(onOpenChange).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("blocks cancel and action while confirmation is pending", () => {
    render(<AlertDialog defaultOpen title="Export" cancel={{ label: "Cancel" }} action={{ label: "Export", pending: true }} />);
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Export" })).toHaveProperty("disabled", true);
  });
});
