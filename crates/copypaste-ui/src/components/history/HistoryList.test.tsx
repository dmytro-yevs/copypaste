/**
 * The list's structural and keyboard contract:
 *
 *   INV-8  / AT-17  role="list" + role="listitem", never listbox/option
 *   INV-9  / AT-12  a polite live region, a **sibling** of the list
 *   INV-7  / AT-11  the active pointer never names an unrendered row
 *   INV-32          selection is tracked by id and re-resolved every render
 *   AT-10           arrow keys clamp at both ends and do not wrap
 *   A11Y-14         the announcer is not inside role="list"
 */
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";

import { HistoryList } from "@/components/history/HistoryList";
import { items } from "@/test/harness";
import type { Item } from "@/lib/ipc";

function setup(count = 5, over: Partial<Parameters<typeof HistoryList>[0]> = {}) {
  const listRef = createRef<HTMLDivElement>();
  const onActiveIdChange = vi.fn();
  const onCopy = vi.fn();
  const onDelete = vi.fn();
  const data = items(count);

  const props = {
    items: data,
    activeId: null as string | null,
    onActiveIdChange,
    revealedId: null,
    revealedContent: null,
    revealPendingId: null,
    previewLines: 2,
    onReveal: vi.fn(),
    onHide: vi.fn(),
    onCopy,
    onTogglePin: vi.fn(),
    onDelete,
    onLoadMore: vi.fn(),
    listRef,
    ...over,
  };

  const view = render(<HistoryList {...props} />);
  return { ...view, props, data, listRef, onActiveIdChange, onCopy, onDelete };
}

describe("list semantics", () => {
  it("is a list of listitems, not a listbox of options (INV-8)", () => {
    setup();
    expect(screen.getByRole("list", { name: "Clipboard history" })).toBeTruthy();
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(screen.getAllByRole("listitem").length).toBeGreaterThan(0);
  });

  it("keeps the live region outside the list (A11Y-14)", () => {
    setup();
    const list = screen.getByRole("list", { name: "Clipboard history" });
    const announcer = screen.getByTestId("history-announcer");
    // A live region inside role="list" fails aria-required-children — this is
    // CopyPaste-wrfn, and it is a sibling relationship, not a style.
    expect(list.contains(announcer)).toBe(false);
    expect(announcer.getAttribute("aria-live")).toBe("polite");
    expect(announcer.getAttribute("aria-atomic")).toBe("true");
  });

  it("exposes selection with aria-current, not aria-selected", () => {
    const { data } = setup(3, { activeId: "row-1" });
    const rows = screen.getAllByRole("listitem");
    const active = rows.find((row) => row.id === `history-row-${data[1]!.id}`);
    expect(active?.getAttribute("aria-current")).toBe("true");
    expect(active?.getAttribute("aria-selected")).toBeNull();
  });

  it("announces the active row's label (AT-12)", () => {
    setup(3, { activeId: "row-2" });
    expect(screen.getByTestId("history-announcer").textContent).toBe("entry 2");
  });

  it("clears the active-descendant pointer for a row that is not rendered (AT-11)", () => {
    // Nothing is rendered for an id that is not in the list, so the pointer
    // must be absent rather than dangling (CopyPaste-5917.33).
    setup(3, { activeId: "row-does-not-exist" });
    const list = screen.getByRole("list", { name: "Clipboard history" });
    expect(list.getAttribute("data-active-descendant")).toBeNull();
  });

  it("points at the active row while it is rendered", () => {
    setup(3, { activeId: "row-1" });
    const list = screen.getByRole("list", { name: "Clipboard history" });
    expect(list.getAttribute("data-active-descendant")).toBe("history-row-row-1");
  });
});

describe("keyboard (AT-10)", () => {
  it("moves down and clamps at the end without wrapping", () => {
    const { listRef, onActiveIdChange } = setup(3, { activeId: "row-2" });
    fireEvent.keyDown(listRef.current!, { key: "ArrowDown" });
    // Already last: stays put rather than wrapping to the first.
    expect(onActiveIdChange).toHaveBeenCalledWith("row-2");
  });

  it("moves up and clamps at the start without wrapping", () => {
    const { listRef, onActiveIdChange } = setup(3, { activeId: "row-0" });
    fireEvent.keyDown(listRef.current!, { key: "ArrowUp" });
    expect(onActiveIdChange).toHaveBeenCalledWith("row-0");
  });

  it("selects the first row when nothing is selected", () => {
    const { listRef, onActiveIdChange } = setup(3);
    fireEvent.keyDown(listRef.current!, { key: "ArrowDown" });
    expect(onActiveIdChange).toHaveBeenCalledWith("row-0");
  });

  it("copies the selected item on Enter", () => {
    const { listRef, onCopy, data } = setup(3, { activeId: "row-1" });
    fireEvent.keyDown(listRef.current!, { key: "Enter" });
    expect(onCopy).toHaveBeenCalledWith(data[1]);
  });

  it("moves the selection before deleting, so it never dangles", () => {
    const { listRef, onActiveIdChange, onDelete, data } = setup(3, {
      activeId: "row-1",
    });
    fireEvent.keyDown(listRef.current!, { key: "Backspace" });
    expect(onActiveIdChange).toHaveBeenCalledWith("row-2");
    expect(onDelete).toHaveBeenCalledWith(data[1]);
  });

  it("resolves the selection by id, not by index (INV-32)", () => {
    // A poll can reorder the list between the keydown and the Enter that
    // follows it; an index would then paste the wrong item (CopyPaste-8ebg.17).
    const data = items(3);
    const reordered: Item[] = [data[2]!, data[0]!, data[1]!];
    const onCopy = vi.fn();
    const { listRef } = setup(3, {
      items: reordered,
      activeId: "row-0",
      onCopy,
    });
    fireEvent.keyDown(listRef.current!, { key: "Enter" });
    expect(onCopy).toHaveBeenCalledWith(data[0]);
  });

  it("clears the selection on Escape", () => {
    const { listRef, onActiveIdChange } = setup(3, { activeId: "row-1" });
    fireEvent.keyDown(listRef.current!, { key: "Escape" });
    expect(onActiveIdChange).toHaveBeenCalledWith(null);
  });
});
