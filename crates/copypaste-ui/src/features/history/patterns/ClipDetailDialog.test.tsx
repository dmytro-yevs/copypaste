import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";
import { ClipDetailDialog } from "./ClipDetailDialog";

describe("ClipDetailDialog notices", () => {
  it("uses shared status notices for sync and sensitive-content warnings", () => {
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({
            too_large_to_sync: true,
            sensitive_finding: {
              label: "possible token",
              spans: [{ start: 0, end: 5 }],
              spans_truncated: false,
              redacted_preview: "••••• content",
            },
          })}
          origin={null}
          initialExpanded
          fullContent="plain content"
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

    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(
      screen
        .getByText("Too large to sync — this item stays on this device")
        .closest('[data-slot="surface"]'),
    ).toBeTruthy();
    expect(screen.getByText("Potentially sensitive content")).toBeTruthy();
  });
});
