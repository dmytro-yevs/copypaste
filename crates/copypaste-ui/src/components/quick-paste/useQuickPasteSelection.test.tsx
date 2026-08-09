import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { KeyboardEvent, RefObject } from "react";

import { useQuickPasteSelection } from "@/components/quick-paste/useQuickPasteSelection";
import { item } from "@/test/harness";

function keyEvent(key: string, modifiers: Partial<KeyboardEvent<HTMLDivElement>> = {}) {
  return {
    key,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    preventDefault: vi.fn(),
    ...modifiers,
  } as KeyboardEvent<HTMLDivElement>;
}

function renderSelection(query = "") {
  const items = [item({ id: "first" }), item({ id: "second" }), item({ id: "third" })];
  const list = document.createElement("div");
  const rows = items.map(() => document.createElement("div"));
  list.append(...rows);
  const listRef = { current: list } satisfies RefObject<HTMLDivElement>;
  const onCopy = vi.fn();
  const onDismiss = vi.fn();
  const rendered = renderHook(
    ({ active, search }) =>
      useQuickPasteSelection({ active, items, query: search, listRef, onCopy, onDismiss }),
    { initialProps: { active: true, search: query } },
  );
  return { ...rendered, items, rows, onCopy, onDismiss };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-08-09T12:00:00Z"));
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useQuickPasteSelection", () => {
  it("wraps ID selection and scrolls only after keyboard movement", () => {
    const { result, items, rows, onCopy } = renderSelection();
    const scrollIntoView = vi.fn();
    rows[2]!.scrollIntoView = scrollIntoView;

    expect(result.current.selectedId).toBe("first");
    act(() => result.current.onKeyDown(keyEvent("ArrowUp")));

    expect(result.current.selectedId).toBe("third");
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
    act(() => result.current.onKeyDown(keyEvent("Enter", { altKey: true })));
    expect(onCopy).toHaveBeenCalledWith(items[2], true);
  });

  it("suppresses pointer selection after keyboard moves and during scrolling", () => {
    const { result } = renderSelection();

    act(() => result.current.onKeyDown(keyEvent("ArrowDown")));
    expect(result.current.selectedId).toBe("second");
    act(() => result.current.selectFromPointer("third"));
    expect(result.current.selectedId).toBe("second");

    act(() => vi.advanceTimersByTime(250));
    act(() => result.current.selectFromPointer("third"));
    expect(result.current.selectedId).toBe("third");

    act(() => result.current.noteScroll());
    act(() => result.current.selectFromPointer("first"));
    expect(result.current.selectedId).toBe("third");
    act(() => vi.advanceTimersByTime(120));
    act(() => result.current.selectFromPointer("first"));
    expect(result.current.selectedId).toBe("first");
  });

  it("dismisses on Escape and disables numbered shortcuts during search", () => {
    const { result, rerender, items, onCopy, onDismiss } = renderSelection("needle");

    act(() => result.current.onKeyDown(keyEvent("Escape")));
    expect(onDismiss).toHaveBeenCalledTimes(1);
    act(() => result.current.onKeyDown(keyEvent("1", { metaKey: true })));
    expect(onCopy).not.toHaveBeenCalled();

    rerender({ active: true, search: "" });
    act(() => result.current.onKeyDown(keyEvent("1", { metaKey: true })));
    expect(onCopy).toHaveBeenCalledWith(items[0]);
  });

  it("drops selection while the popup cache is inactive", () => {
    const { result, rerender } = renderSelection();

    act(() => result.current.onKeyDown(keyEvent("ArrowDown")));
    expect(result.current.selectedId).toBe("second");

    rerender({ active: false, search: "" });
    expect(result.current.selectedId).toBeNull();
    rerender({ active: true, search: "" });
    expect(result.current.selectedId).toBe("first");
  });
});
