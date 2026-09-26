import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { t } from "@/i18n";
import { item } from "@/test/harness";
import { ClipDetailDialog } from "./ClipDetailDialog";

vi.mock("@/features/clip-content/hooks/useImagePreview", () => ({
  useImagePreview: () => ({ data: undefined, isPending: true, isError: false }),
}));

describe("ClipDetailDialog notices", () => {
  it("keeps a potential original redacted until shown in the reader", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({
            content: "original value",
            sensitive_finding: {
              label: "possible secret",
              spans: [{ start: 0, end: 8 }],
              spans_truncated: false,
              redacted_preview: "redacted value",
            },
          })}
          origin={null}
          initialExpanded
          fullContent="original value"
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

    const contents = screen.getByRole("region", { name: "Item contents" });
    expect(contents.textContent).toBe("redacted value");
    await user.click(screen.getByRole("button", { name: "Show original content" }));
    expect(contents.textContent).toBe("original value");
    await user.click(screen.getByRole("button", { name: "Hide original content" }));
    expect(contents.textContent).toBe("redacted value");
  });

  it("keeps the reader open until copy succeeds", async () => {
    const user = userEvent.setup();
    let finish!: () => void;
    const onCopy = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    const onClose = vi.fn();
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item()}
          origin={null}
          initialExpanded
          fullContent={null}
          fullContentFailed={false}
          revealedContent={null}
          revealPending={false}
          onReveal={vi.fn()}
          onHide={vi.fn()}
          onCopy={onCopy}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled")).toBe(true);
    finish();
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("keeps the reader actionable after a failed copy", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item()}
          origin={null}
          initialExpanded
          fullContent={null}
          fullContentFailed={false}
          revealedContent={null}
          revealPending={false}
          onReveal={vi.fn()}
          onHide={vi.fn()}
          onCopy={vi.fn().mockRejectedValue(new Error("copy failed"))}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() =>
      expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
        .toBe(false),
    );
    expect(onClose).not.toHaveBeenCalled();
  });

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
    expect(t("history.inspector.tooLarge")).toBe(
      "Too large to sync — this item stays on this device",
    );
    expect(t("history.inspector.tooLarge")).not.toBe(
      "Too large · peer sync only",
    );
    const syncNotice = screen
      .getAllByText("Too large to sync — this item stays on this device")
      .map((element) => element.closest<HTMLElement>('[data-slot="surface"]'))
      .find((element) => element !== null);
    expect(syncNotice).toBeTruthy();
    expect(
      within(syncNotice!).getByText(
        "Too large to sync — this item stays on this device",
      ),
    ).toBeTruthy();
    expect(screen.getByText("Potentially sensitive content")).toBeTruthy();
  });

  it("uses the shared unavailable state instead of a failed body preview", () => {
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({ content: "short preview", truncated: true })}
          origin={null}
          initialExpanded
          fullContent={null}
          fullContentFailed
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

    const unavailable = screen.getByRole("status");
    expect(unavailable.textContent).toBe("Full contents could not be loaded.");
    expect(screen.queryByText("short preview")).toBeNull();
    expect(unavailable.getAttribute("data-slot")).toBe("preview-surface");
  });

  it("uses singular metadata and the shared image copy action", () => {
    render(
      <TooltipProvider>
        <ClipDetailDialog
          item={item({
            content: "image",
            content_class: "image",
            content_type: "image/png",
          })}
          origin={null}
          initialExpanded
          fullContent={null}
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

    expect(screen.getByText(/ · Image$/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy image" })).toBeTruthy();
  });
});
