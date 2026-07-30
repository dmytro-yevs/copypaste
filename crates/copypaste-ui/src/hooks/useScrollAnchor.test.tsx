/**
 * INV-1 / INV-6 — content-anchored scrolling and the shrink clamp
 * (AT-1, AT-2, AT-3, AT-4).
 *
 * Manifest §9.1 is explicit that TanStack Virtual does **not** give you INV-1
 * for a mutating list, so the anchoring arithmetic is ours and this is where it
 * is checked. The virtualizer is a stand-in with the four members the hook
 * actually uses: jsdom cannot lay anything out, so measuring a real one would
 * only assert that zero equals zero.
 */
import { describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { createRef } from "react";
import type { Virtualizer } from "@tanstack/react-virtual";

import { useScrollAnchor } from "@/hooks/useScrollAnchor";
import { rowHeight } from "@/lib/layout";
import type { Item } from "@/lib/ipc";
import { items } from "@/test/harness";

const ROW = rowHeight(2);
const VIEWPORT = 400;

/** A virtualizer whose geometry is a fixed row height — the same model the
 *  real one is configured with (INV-5). */
function fakeVirtualizer(
  list: readonly Item[],
  previewLines: number,
  scrollToOffset = vi.fn(),
) {
  // The row height tracks the setting, exactly as `estimateSize` does — which
  // is what makes lowering it shrink total content height (AT-4).
  const size = rowHeight(previewLines);
  const measurementsCache = list.map((entry, index) => ({
    index,
    key: entry.id,
    start: index * size,
    end: (index + 1) * size,
    size,
    lane: 0,
  }));

  return {
    virtualizer: {
      getTotalSize: () => list.length * size,
      measurementsCache,
      getVirtualItemForOffset: (offset: number) =>
        measurementsCache[Math.min(list.length - 1, Math.floor(offset / size))],
      scrollToOffset,
    } as unknown as Virtualizer<HTMLDivElement, Element>,
    scrollToOffset,
  };
}

function setup(list: readonly Item[], scrollTop: number, previewLines = 2) {
  const el = document.createElement("div");
  el.style.height = `${VIEWPORT}px`;
  document.body.appendChild(el);
  el.scrollTop = scrollTop;

  const scrollRef = createRef<HTMLDivElement>();
  (scrollRef as { current: HTMLDivElement }).current = el;

  const scrollToOffset = vi.fn((offset: number) => {
    el.scrollTop = offset;
  });

  const rendered = renderHook(
    ({ current, lines }: { current: readonly Item[]; lines: number }) =>
      useScrollAnchor({
        scrollRef,
        virtualizer: fakeVirtualizer(current, lines, scrollToOffset).virtualizer,
        items: current,
        previewLines: lines,
      }),
    { initialProps: { current: list, lines: previewLines } },
  );

  return { el, scrollToOffset, ...rendered };
}

describe("anchoring across a mutation (AT-1, AT-2)", () => {
  it("keeps the anchored row under the same point when a row is prepended", () => {
    const list = items(20);
    const { el, result, rerender, scrollToOffset } = setup(list, ROW * 5);

    // The user is looking at row 5, flush with the top of the viewport.
    result.current.captureAnchor();

    const prepended = [items(1)[0]!, ...list].map((entry, index) =>
      index === 0 ? { ...entry, id: "brand-new" } : entry,
    );
    rerender({ current: prepended, lines: 2 });

    // Row 5 moved down one slot, so the viewport follows it: no visible jump.
    expect(scrollToOffset).toHaveBeenCalledWith(
      ROW * 6,
      expect.objectContaining({ align: "start" }),
    );
    expect(el.scrollTop).toBe(ROW * 6);
  });

  it("preserves an intra-row offset, not just the row", () => {
    const list = items(20);
    const { result, rerender, scrollToOffset } = setup(list, ROW * 5 + 17);
    result.current.captureAnchor();

    rerender({ current: [items(1)[0]!, ...list], lines: 2 });
    expect(scrollToOffset).toHaveBeenCalledWith(ROW * 6 + 17, expect.anything());
  });

  it("leaves a list scrolled to the very top alone", () => {
    // At the top, anchoring would push the new item out of sight — the
    // opposite of what a prepend should do.
    const list = items(20);
    const { result, rerender, scrollToOffset } = setup(list, 0);
    result.current.captureAnchor();
    rerender({ current: [items(1)[0]!, ...list], lines: 2 });
    expect(scrollToOffset).not.toHaveBeenCalled();
  });

  it("appends below without moving the viewport (AT-2)", () => {
    const list = items(20);
    const { result, rerender, scrollToOffset } = setup(list, ROW * 5);
    result.current.captureAnchor();

    const appended = [...list, ...items(20).map((e) => ({ ...e, id: `x-${e.id}` }))];
    rerender({ current: appended, lines: 2 });

    // The anchored row did not move, so nothing is scrolled.
    expect(scrollToOffset).not.toHaveBeenCalled();
  });
});

describe("the anchor row disappearing (AT-3)", () => {
  it("stays in bounds and throws nothing when the anchor is the deleted row", () => {
    const list = items(20);
    const { el, result, rerender } = setup(list, ROW * 5);
    result.current.captureAnchor();

    const without = list.filter((entry) => entry.id !== "row-5");
    expect(() => rerender({ current: without, lines: 2 })).not.toThrow();

    const max = without.length * ROW - VIEWPORT;
    expect(el.scrollTop).toBeGreaterThanOrEqual(0);
    expect(el.scrollTop).toBeLessThanOrEqual(max);
  });
});

describe("the shrink clamp (INV-6 / AT-4)", () => {
  it("clamps immediately when a filter shrinks the list under the offset", () => {
    const list = items(60);
    const { el, result, rerender, scrollToOffset } = setup(list, ROW * 50);
    result.current.captureAnchor();

    rerender({ current: items(4), lines: 2 });

    const max = 4 * ROW - VIEWPORT;
    // Not on the next user scroll — now. Otherwise the visible window
    // degenerates to the last row or two (CopyPaste-f2ec #17).
    expect(scrollToOffset).toHaveBeenCalled();
    expect(el.scrollTop).toBe(Math.max(0, max));
  });

  it("clamps when the preview-line setting shrinks every row", () => {
    // The one mutation that changes total height without changing the list.
    const list = items(60);
    const { result, rerender, scrollToOffset } = setup(list, ROW * 50, 2);
    result.current.captureAnchor();

    rerender({ current: list, lines: 1 });
    expect(scrollToOffset).toHaveBeenCalled();
  });

  it("never scrolls to a negative offset", () => {
    const list = items(3);
    const { el, result, rerender } = setup(list, ROW * 2);
    result.current.captureAnchor();
    rerender({ current: items(1), lines: 2 });
    expect(el.scrollTop).toBe(0);
  });
});
