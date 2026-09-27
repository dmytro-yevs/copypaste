import { fireEvent, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Item } from "@/lib/ipc";
import { item } from "@/test/harness";
import { useQuickPasteSelection } from "./useQuickPasteSelection";

function SelectionProbe({
  items,
  query,
  sessionKey,
  active = true,
  onLoadMore,
}: {
  items: readonly Item[];
  query: string;
  sessionKey: number;
  active?: boolean;
  onLoadMore: () => void;
}) {
  const selection = useQuickPasteSelection({
    active,
    items,
    query,
    sessionKey,
    scrollToItemIndex: vi.fn(),
    hasMore: true,
    onLoadMore,
    canCopy: () => true,
    copyPending: false,
    onCopy: vi.fn(),
    onDismiss: vi.fn(),
  });
  return <div data-selected={selection.selectedId ?? ""} onKeyDown={selection.onKeyDown} />;
}

describe("useQuickPasteSelection", () => {
  it("continues after its anchor when a refreshed head prepends before an older page", async () => {
    const anchor = item({ id: "anchor" });
    const newer = item({ id: "newer" });
    const older = item({ id: "older" });
    const loadMore = vi.fn();
    const view = render(<SelectionProbe items={[anchor]} query="" sessionKey={1} onLoadMore={loadMore} />);
    const probe = view.container.firstElementChild!;
    await waitFor(() => expect(probe.getAttribute("data-selected")).toBe("anchor"));
    fireEvent.keyDown(probe, { key: "ArrowDown" });
    expect(loadMore).toHaveBeenCalledOnce();

    view.rerender(<SelectionProbe items={[newer, anchor, older]} query="" sessionKey={1} onLoadMore={loadMore} />);
    await waitFor(() => expect(probe.getAttribute("data-selected")).toBe("older"));
  });

  it("drops a pending continuation when the query or popup session changes", async () => {
    const anchor = item({ id: "anchor" });
    const replacement = item({ id: "replacement" });
    const older = item({ id: "older" });
    const loadMore = vi.fn();
    const view = render(<SelectionProbe items={[anchor]} query="old" sessionKey={1} onLoadMore={loadMore} />);
    const probe = view.container.firstElementChild!;
    await waitFor(() => expect(probe.getAttribute("data-selected")).toBe("anchor"));
    fireEvent.keyDown(probe, { key: "ArrowDown" });

    view.rerender(<SelectionProbe items={[replacement]} query="new" sessionKey={2} onLoadMore={loadMore} />);
    await waitFor(() => expect(probe.getAttribute("data-selected")).toBe("replacement"));
    view.rerender(<SelectionProbe items={[replacement, older]} query="new" sessionKey={2} onLoadMore={loadMore} />);

    expect(probe.getAttribute("data-selected")).toBe("replacement");
  });
});
