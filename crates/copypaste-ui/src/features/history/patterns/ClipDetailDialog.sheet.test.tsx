import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";
import { ClipDetailDialog } from "./ClipDetailDialog";

vi.mock("@/hooks/useClipboardWriteAvailability", () => ({
  useClipboardWriteAvailability: () => ({ isPending: false, isError: false, data: "available" }),
}));

vi.mock(import("@/hooks/useViewportMetrics"), async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    useViewportMetrics: () => ({
      width: 390,
      height: 844,
      pointer: "coarse" as const,
      sizeClass: "compact" as const,
    }),
  };
});

vi.mock("@/features/history/hooks/useImagePreview", () => ({
  useImagePreview: () => ({ data: undefined, isPending: false, isError: false }),
}));

describe("ClipDetailDialog compact sheet", () => {
  it("keeps the compact inspector open after a successful copy", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const onCopy = vi.fn().mockResolvedValue(undefined);
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "sheet copy" })}
          origin={null}
          fullContent="sheet copy"
          fullContentFailed={false}
          onCopy={onCopy}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(onCopy).toHaveBeenCalledOnce());
    await vi.waitFor(() =>
      expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
        .toBe(false),
    );
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Clipboard item" })).toBeTruthy();
  });

  it("dismisses a sheet drag when no copy is pending", () => {
    const onClose = vi.fn();
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "sheet close" })}
          origin={null}
          initialExpanded
          fullContent="sheet close"
          fullContentFailed={false}
          onCopy={vi.fn()}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    const handle = document.querySelector<HTMLElement>('[data-slot="dialog-sheet-handle"]');
    expect(handle).not.toBeNull();
    fireEvent.pointerDown(handle!, { button: 0, pointerId: 1, clientY: 0 });
    fireEvent.pointerMove(handle!, { pointerId: 1, clientY: 100 });
    fireEvent.pointerUp(handle!, { pointerId: 1, clientY: 100 });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("keeps a pending copy open through sheet drag dismissal", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    let finish!: () => void;
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "sheet copy" })}
          origin={null}
          initialExpanded
          fullContent="sheet copy"
          fullContentFailed={false}
          onCopy={() => new Promise<void>((resolve) => { finish = resolve; })}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    const handle = document.querySelector<HTMLElement>('[data-slot="dialog-sheet-handle"]');
    expect(handle).not.toBeNull();
    fireEvent.pointerDown(handle!, { button: 0, pointerId: 1, clientY: 0 });
    fireEvent.pointerMove(handle!, { pointerId: 1, clientY: 100 });
    fireEvent.pointerUp(handle!, { pointerId: 1, clientY: 100 });
    expect(onClose).not.toHaveBeenCalled();
    finish();
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("exposes a swipe handle on the long-clip sheet", () => {
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "a long clip that needs to scroll" })}
          origin={null}
          fullContent="a long clip that needs to scroll"
          fullContentFailed={false}
          onCopy={vi.fn()}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={vi.fn()}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    expect(document.querySelector("[data-slot='dialog-sheet-handle']")).toBeTruthy();
    expect(document.querySelector("[data-slot='dialog-content']")).toBeTruthy();
  });
});
