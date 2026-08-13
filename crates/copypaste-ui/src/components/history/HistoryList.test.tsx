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
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { createRef } from "react";

import { HistoryList } from "@/components/history/HistoryList";
import { quickSlot } from "@/components/history/historyKeyboard";
import { items, withClient } from "@/test/harness";
import type { Selection } from "@/hooks/useSelection";
import type { Item } from "@/lib/ipc";

// DMY-137: virtual-core's scroll debounce is not cancelled on observer
// teardown; the orphaned timer hits the unmounted virtualizer.
beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
afterEach(() => vi.useRealTimers());

/** A selection that records what was asked of it, so a test can assert on the
 *  gesture rather than on the hook's internals. */
function fakeSelection(over: Partial<Selection> = {}): Selection {
  return {
    selecting: false,
    selected: new Set<string>(),
    items: [],
    allPinned: false,
    begin: vi.fn(),
    end: vi.fn(),
    toggle: vi.fn(),
    selectAll: vi.fn(),
    ...over,
  };
}

function setup(count = 5, over: Partial<Parameters<typeof HistoryList>[0]> = {}) {
  const listRef = createRef<HTMLDivElement>();
  const onActiveIdChange = vi.fn();
  const onCopy = vi.fn();
  const onQuickCopy = vi.fn();
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
    onCopy,
    onQuickCopy,
    onTogglePin: vi.fn(),
    onReorderPinned: vi.fn(),
    onDelete,
    onOpen: vi.fn(),
    onLoadMore: vi.fn(),
    listRef,
    searching: false,
    groupedByDevice: false,
    selection: fakeSelection(),
    hasMore: false,
    loadingMore: false,
    ...over,
  };

  const view = withClient(<HistoryList {...props} />);
  return {
    ...view,
    props,
    data,
    listRef,
    onActiveIdChange,
    onCopy,
    onQuickCopy,
    onDelete,
  };
}

describe("list semantics", () => {
  it("exposes a named touch drag handle only for pinned rows", () => {
    const data = items(2).map((item, index) => ({ ...item, pinned: index === 0 }));
    setup(2, { items: data, activeId: data[0]!.id });

    const handle = screen.getByRole("button", { name: "Reorder pinned item" });
    expect(handle.title).toBe("Reorder pinned item");
    expect(handle.className).toContain("touch-none");
    expect(screen.getAllByRole("button", { name: "Reorder pinned item" })).toHaveLength(1);
  });

  it.each(["macOS", "Android"])(
    "reorders pinned rows with Option/Alt+Arrow on %s",
    () => {
      const data = items(3).map((item) => ({ ...item, pinned: true }));
      const onReorderPinned = vi.fn();
      setup(3, { items: data, activeId: data[1]!.id, onReorderPinned });

      fireEvent.keyDown(screen.getByRole("list"), { key: "ArrowUp", altKey: true });

      expect(onReorderPinned).toHaveBeenCalledWith([
        data[1]!.id,
        data[0]!.id,
        data[2]!.id,
      ]);
      expect(screen.getByTestId("history-announcer").textContent).toContain("moved up");
    },
  );

  it("does not reorder an unpinned row or move a pin past an edge", () => {
    const data = items(3).map((item, index) => ({ ...item, pinned: index < 2 }));
    const onReorderPinned = vi.fn();
    const view = setup(3, { items: data, activeId: data[0]!.id, onReorderPinned });
    const list = screen.getByRole("list");

    fireEvent.keyDown(list, { key: "ArrowUp", altKey: true });
    view.rerender(
      <HistoryList
        {...view.props}
        items={data}
        activeId={data[2]!.id}
        onReorderPinned={onReorderPinned}
      />,
    );
    fireEvent.keyDown(list, { key: "ArrowDown", altKey: true });

    expect(onReorderPinned).not.toHaveBeenCalled();
  });

  it("is a list of listitems, not a listbox of options (INV-8)", () => {
    setup();
    expect(screen.getByRole("list", { name: "Clipboard history" })).toBeTruthy();
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(screen.getAllByRole("listitem").length).toBeGreaterThan(0);
  });

  it("puts the list name, focus, and scrolling on one element (A11Y-1)", () => {
    const { listRef } = setup();
    const list = screen.getByRole("list", { name: "Clipboard history" });

    expect(list).toBe(listRef.current);
    expect(list.tabIndex).toBe(0);
    expect(list.className).toContain("overflow-y-auto");

    list.focus();
    expect(document.activeElement).toBe(list);
  });

  it("does not put listbox-only selection state on the list (AT-17)", () => {
    setup(3, { selection: fakeSelection({ selecting: true }) });
    const list = screen.getByRole("list", { name: "Clipboard history" });
    expect(list.getAttribute("aria-multiselectable")).toBeNull();
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

  it("keeps the selected pinned row current while a drag is canceled", async () => {
    const data = items(2).map((item) => ({ ...item, pinned: true }));
    setup(2, { items: data, activeId: data[1]!.id });
    const handle = screen.getAllByRole("button", {
      name: "Reorder pinned item",
    })[1]!;

    fireEvent.keyDown(handle, { key: "Enter", code: "Enter" });
    await waitFor(() =>
      expect(
        document
          .getElementById(`history-row-${data[1]!.id}`)
          ?.hasAttribute("data-dnd-dragging"),
      ).toBe(true),
    );
    fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });

    expect(document.getElementById(`history-row-${data[1]!.id}`)?.getAttribute("aria-current"))
      .toBe("true");
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

  it("renders device headings inside the virtualized list when grouped", () => {
    const data = [
      {
        ...items(1)[0]!,
        id: "mac",
        origin_device_id: "device-a",
        origin_device_name: "Mac",
      },
      {
        ...items(1)[0]!,
        id: "phone",
        origin_device_id: "device-b",
        origin_device_name: "Phone",
      },
    ] as unknown as Item[];
    setup(2, { items: data, groupedByDevice: true });
    expect(screen.getByRole("heading", { name: "Mac" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Phone" })).toBeTruthy();
  });

  it("keeps the device label when the cap leaves one visible group", () => {
    const data = [
      {
        ...items(1)[0]!,
        origin_device_id: "device-a",
        origin_device_name: "Mac",
      },
    ] as unknown as Item[];
    setup(1, { items: data, groupedByDevice: true });
    expect(screen.getByRole("heading", { name: "Mac" })).toBeTruthy();
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

  it("opens the detail view for the selected row on ArrowRight", () => {
    const onOpen = vi.fn();
    const { listRef, data } = setup(3, { activeId: "row-1", onOpen });
    fireEvent.keyDown(listRef.current!, { key: "ArrowRight" });
    expect(onOpen).toHaveBeenCalledWith(data[1]);
  });

  it("clears the selection on Escape", () => {
    const { listRef, onActiveIdChange } = setup(3, { activeId: "row-1" });
    fireEvent.keyDown(listRef.current!, { key: "Escape" });
    expect(onActiveIdChange).toHaveBeenCalledWith(null);
  });
});

describe("⌘1–⌘9 quick copy", () => {
  /**
   * The fastest path in a clipboard manager: hotkey, glance, ⌘3. v1 had it and
   * v2 did not (manifest 06 §3.5.3).
   */
  it("copies the nth row and asks for the window to go away", () => {
    const { data, onQuickCopy, onCopy, listRef } = setup(5);
    fireEvent.keyDown(listRef.current!, { key: "3", metaKey: true });
    expect(onQuickCopy).toHaveBeenCalledWith(data[2]);
    // Not the ordinary copy path: that one leaves the window up.
    expect(onCopy).not.toHaveBeenCalled();
  });

  it("does nothing for a slot with no row behind it", () => {
    const { onQuickCopy, listRef } = setup(2);
    fireEvent.keyDown(listRef.current!, { key: "7", metaKey: true });
    expect(onQuickCopy).not.toHaveBeenCalled();
  });

  /**
   * Inactive while searching. The digits address positions in the history, and
   * a filtered list renumbers them under the user's fingers between keystrokes
   * — so ⌘3 would copy a different item depending on how fast they typed.
   */
  it("is inactive while a search is running", () => {
    const { onQuickCopy, listRef } = setup(5, { searching: true });
    fireEvent.keyDown(listRef.current!, { key: "1", metaKey: true });
    expect(onQuickCopy).not.toHaveBeenCalled();
    expect(quickSlot("1", true)).toBeNull();
    expect(quickSlot("1", false)).toBe(0);
  });

  it("rejects a digit outside the nine slots", () => {
    expect(quickSlot("0", false)).toBeNull();
    expect(quickSlot("9", false)).toBe(8);
    expect(quickSlot("a", false)).toBeNull();
  });
});

describe("selection mode", () => {
  it("⌘A selects everything currently on screen", () => {
    const selection = fakeSelection();
    const { listRef } = setup(4, { selection });
    fireEvent.keyDown(listRef.current!, { key: "a", metaKey: true });
    expect(selection.selectAll).toHaveBeenCalledTimes(1);
  });

  /** §3.1.4: Escape clears the multi-selection first, and only then the single
   *  one — otherwise leaving selection mode takes two presses that look alike. */
  it("Escape leaves selection mode before it clears the active row", () => {
    const selection = fakeSelection({ selecting: true });
    const { listRef, onActiveIdChange } = setup(3, { selection });
    fireEvent.keyDown(listRef.current!, { key: "Escape" });
    expect(selection.end).toHaveBeenCalledTimes(1);
    expect(onActiveIdChange).not.toHaveBeenCalled();
  });

  /** Delete must not act on the active row while a multi-selection is live:
   *  the bulk bar's Delete is the one that means the selection. */
  it("Delete does nothing while selecting", () => {
    const rows = items(3);
    const selection = fakeSelection({ selecting: true });
    const { listRef, onDelete } = setup(3, {
      selection,
      items: rows,
      activeId: rows[0]!.id,
    });
    fireEvent.keyDown(listRef.current!, { key: "Delete" });
    expect(onDelete).not.toHaveBeenCalled();
  });
});

describe("load more", () => {
  it("keeps list scrolling active when pinned rows have touch handles", () => {
    const onLoadMore = vi.fn();
    const data = items(2).map((item) => ({ ...item, pinned: true }));
    const { listRef } = setup(2, { items: data, onLoadMore });
    Object.defineProperties(listRef.current!, {
      clientHeight: { configurable: true, value: 400 },
      scrollHeight: { configurable: true, value: 1000 },
      scrollTop: { configurable: true, writable: true, value: 550 },
    });

    fireEvent.scroll(listRef.current!);

    expect(listRef.current!.className).toContain("overflow-y-auto");
    expect(onLoadMore).toHaveBeenCalledOnce();
  });

  /**
   * A filtered list can be three rows long with a thousand unpaged matches
   * behind it, and three rows do not scroll — so the near-bottom trigger alone
   * strands the user on a result that looks empty and is not.
   */
  it("offers an explicit control when there are more pages", () => {
    const { props } = setup(3, { hasMore: true });
    const button = screen.getByRole("button", { name: /load more/i });
    fireEvent.click(button);
    expect(props.onLoadMore).toHaveBeenCalled();
  });

  it("offers nothing when the history is fully loaded", () => {
    setup(3, { hasMore: false });
    expect(screen.queryByRole("button", { name: /load more/i })).toBeNull();
  });
});
