import type { ReactElement, MouseEvent } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import { UndoCountdown } from "@/components/shared/UndoCountdown";
import { toast } from "@/lib/notify";
import type { StateViewProps } from "@/components/shared/StateView";

const sonnerMock = vi.hoisted(() => ({
  entries: [] as Array<{
    id: string | number;
    element: unknown;
    options: Record<string, unknown>;
  }>,
  dismiss: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: {
    custom: (
      renderToast: (id: string | number) => unknown,
      options: Record<string, unknown> = {},
    ) => {
      const id = options.id ?? `generated-${sonnerMock.entries.length}`;
      sonnerMock.entries.push({ id: id as string | number, element: renderToast(id as string | number), options });
      return id;
    },
    dismiss: sonnerMock.dismiss,
  },
}));

beforeEach(() => {
  sonnerMock.entries.length = 0;
  sonnerMock.dismiss.mockReset();
});

function latestToast() {
  const entry = sonnerMock.entries[sonnerMock.entries.length - 1];
  if (!entry) throw new Error("Expected a toast to be created");
  return entry;
}

function renderLatestToast() {
  return render(latestToast().element as ReactElement<StateViewProps>, {
    wrapper: TooltipProvider,
  });
}

describe("toast StateView adapter", () => {
  it("preserves id, duration, dismissibility, callbacks, description, and both actions", () => {
    const onDismiss = vi.fn();
    const onAutoClose = vi.fn();
    const onAction = vi.fn();
    const onCancel = vi.fn();

    expect(toast.error("Could not save", {
      id: "save-toast",
      duration: 4_321,
      dismissible: false,
      description: "Try again in a moment.",
      onDismiss,
      onAutoClose,
      action: { label: "Retry", onClick: onAction },
      cancel: { label: "Keep", onClick: onCancel },
    })).toBe("save-toast");

    const entry = latestToast();
    expect(entry.options).toMatchObject({
      id: "save-toast",
      duration: 4_321,
      dismissible: false,
      onDismiss,
      onAutoClose,
      type: "error",
      icon: null,
    });
    expect(entry.options).not.toHaveProperty("action");
    expect(entry.options).not.toHaveProperty("cancel");
    expect(entry.options).not.toHaveProperty("description");

    const state = entry.element as ReactElement<StateViewProps>;
    expect(state.props).toMatchObject({
      mode: "error",
      role: "group",
      "aria-live": "off",
      title: "Could not save",
      description: "Try again in a moment.",
    });

    renderLatestToast();
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Keep" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Close" }).hasAttribute("disabled")).toBe(true);
  });

  it("keeps UndoCountdown in the description and honors preventDefault on actions", () => {
    const onUndo = vi.fn((event: MouseEvent<HTMLButtonElement>) => event.preventDefault());
    const id = toast("Deleted", {
      id: "undo-toast",
      description: <UndoCountdown ms={5_000} />,
      action: { label: "Undo", onClick: onUndo },
    });

    const state = latestToast().element as ReactElement<StateViewProps>;
    expect((state.props.description as ReactElement).type).toBe(UndoCountdown);
    expect(id).toBe("undo-toast");

    renderLatestToast();
    expect(screen.getByText("Undo — 5 seconds to change your mind")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(onUndo).toHaveBeenCalledOnce();
    expect(sonnerMock.dismiss).not.toHaveBeenCalled();
  });

  it("dismisses after an unprevented action, cancel, and the accessible close button", () => {
    const onAction = vi.fn();
    const onCancel = vi.fn();

    toast.warning("Partial sync", {
      id: "action-toast",
      action: { label: "Review", onClick: onAction },
    });
    renderLatestToast();
    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    expect(onAction).toHaveBeenCalledOnce();
    expect(sonnerMock.dismiss).toHaveBeenCalledWith("action-toast");
    cleanup();

    toast.info("Pairing is ready", {
      id: "cancel-toast",
      cancel: { label: "Cancel", onClick: onCancel },
    });
    renderLatestToast();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(sonnerMock.dismiss).toHaveBeenCalledWith("cancel-toast");
    cleanup();

    toast.loading("Working", { id: "loading-toast" });
    expect(latestToast().options).toMatchObject({ id: "loading-toast", type: "loading" });

    toast.success("Saved", { id: "close-toast", closeButton: true });
    renderLatestToast();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(sonnerMock.dismiss).toHaveBeenCalledWith("close-toast");

    toast.dismiss("explicit-toast");
    expect(sonnerMock.dismiss).toHaveBeenLastCalledWith("explicit-toast");
  });
});
