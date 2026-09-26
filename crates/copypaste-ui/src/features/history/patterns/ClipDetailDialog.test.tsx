import { act, render, screen, within } from "@testing-library/react";
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

  it("blocks Escape, close, backdrop and mutations until copy succeeds", async () => {
    const user = userEvent.setup();
    let finish!: () => void;
    const onCopy = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    const onClose = vi.fn();
    const onTogglePin = vi.fn();
    const onDelete = vi.fn();
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
          onTogglePin={onTogglePin}
          onDelete={onDelete}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(onCopy).toHaveBeenCalledOnce());
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Pin item" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Delete item" }).hasAttribute("disabled")).toBe(true);
    await user.click(screen.getByRole("button", { name: "Pin item" }));
    await user.click(screen.getByRole("button", { name: "Delete item" }));
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Close" }));
    const backdrop = document.querySelector<HTMLElement>('[data-slot="dialog-overlay"]');
    expect(backdrop).not.toBeNull();
    await user.click(backdrop!);
    expect(onClose).not.toHaveBeenCalled();
    const dialog = screen.getByRole("dialog", { name: "Clipboard item" });
    await vi.waitFor(() =>
      expect(dialog.contains(document.activeElement)).toBe(true),
    );
    expect(onTogglePin).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
    finish();
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("keeps the reader actionable after a failed copy", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    let fail!: (reason: Error) => void;
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
          onCopy={() => new Promise<void>((_resolve, reject) => { fail = reject; })}
          onTogglePin={vi.fn()}
          onDelete={vi.fn()}
          onClose={onClose}
          onReturnFocus={vi.fn()}
        />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(fail).toBeTypeOf("function"));
    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
    fail(new Error("copy failed"));
    await vi.waitFor(() =>
      expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
        .toBe(false),
    );
    expect(onClose).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("ignores an old copy result after another item starts copying", async () => {
    const user = userEvent.setup();
    const first = item({ id: "old", content: "old body" });
    const second = item({ id: "new", content: "new body" });
    const finishes = new Map<string, () => void>();
    const onCopy = vi.fn((target: typeof first) =>
      new Promise<void>((resolve) => finishes.set(target.id, resolve)),
    );
    const onClose = vi.fn();
    const common = {
      origin: null,
      initialExpanded: true,
      fullContent: null,
      fullContentFailed: false,
      revealedContent: null,
      revealPending: false,
      onReveal: vi.fn(),
      onHide: vi.fn(),
      onCopy,
      onTogglePin: vi.fn(),
      onDelete: vi.fn(),
      onClose,
      onReturnFocus: vi.fn(),
    };
    const { rerender } = render(
      <TooltipProvider><ClipDetailDialog {...common} item={first} /></TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(finishes.has("old")).toBe(true));
    rerender(
      <TooltipProvider><ClipDetailDialog {...common} item={second} /></TooltipProvider>,
    );
    expect(screen.getByRole("region", { name: "Item contents" }).textContent)
      .toBe("new body");
    await user.click(screen.getByRole("button", { name: "Copy" }));
    await vi.waitFor(() => expect(finishes.has("new")).toBe(true));
    await act(async () => { finishes.get("old")!(); });
    expect(screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
      .toBe(true);
    expect(onClose).not.toHaveBeenCalled();
    finishes.get("new")!();
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
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
