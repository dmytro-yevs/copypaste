import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { QuickPasteRow, quickPasteRowLabel } from "@/features/quick-paste/components/QuickPasteRow";
import { TooltipProvider } from "@/components/ui";
import { item } from "@/test/harness";

const unsupported = item({
  content: "https://future.example/raw",
  content_type: "application/x-future",
  content_class: "other",
});

describe("QuickPasteRow", () => {
  it("keeps unsupported payload text out of its body and copy label", () => {
    render(
      <TooltipProvider>
        <QuickPasteRow
          item={unsupported}
          active
          previewLines={2}
          shortcut={null}
          pinPending={false}
          origin={null}
          fullContent={null}
          fullContentFailed={false}
          onSelect={() => {}}
          onCopy={() => {}}
          onTogglePin={() => {}}
        />
      </TooltipProvider>,
    );

    expect(screen.getByText("Unsupported clipboard content")).not.toBeNull();
    expect(screen.queryByText(unsupported.content!)).toBeNull();
    expect(screen.getByRole("button", { name: "Copy Unsupported clipboard content" })).not.toBeNull();
  });

  it("uses the localized unsupported label", () => {
    expect(quickPasteRowLabel(unsupported)).toBe("Unsupported clipboard content");
  });
});
