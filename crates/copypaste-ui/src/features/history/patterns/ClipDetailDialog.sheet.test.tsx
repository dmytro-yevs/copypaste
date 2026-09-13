import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";
import { ClipDetailDialog } from "./ClipDetailDialog";

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
  it("exposes a swipe handle on the long-clip sheet", () => {
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "a long clip that needs to scroll" })}
          origin={null}
          fullContent="a long clip that needs to scroll"
          fullContentFailed={false}
          revealedContent={null}
          revealPending={false}
          onReveal={vi.fn()}
          onHide={vi.fn()}
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
