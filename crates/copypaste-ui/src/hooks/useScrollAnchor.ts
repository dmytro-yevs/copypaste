/**
 * INV-1 (content-anchored scrolling) and INV-6 (the shrink clamp). §9.1 is
 * explicit that TanStack Virtual does not give you either for a list that
 * mutates, so the arithmetic is ours.
 *
 * The anchor is **item id + intra-row offset** — never an index, because a poll
 * can reorder between two frames (CopyPaste-8ebg.44). If the anchor row is gone
 * it was the one deleted (AT-3): fall through and clamp, never throw.
 *
 * One deliberate exception: at `scrollTop === 0` nothing is anchored, so a
 * prepend reveals the new item instead of pushing it out of view.
 */
import { type RefObject, useCallback, useLayoutEffect, useRef } from "react";
import type { Virtualizer } from "@tanstack/react-virtual";

import type { Item } from "@/lib/ipc";

interface Anchor {
  readonly id: string;
  /** Distance from the top of the anchored row to the top of the viewport. */
  readonly offset: number;
}

interface Options {
  scrollRef: RefObject<HTMLDivElement | null>;
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  items: readonly Item[];
  /** Here for INV-6, not for measurement: it is the one mutation that shrinks
   *  total height without changing the list, and AT-4 wants the clamp *now*. */
  previewLines: number;
}

export function useScrollAnchor({
  scrollRef,
  virtualizer,
  items,
  previewLines,
}: Options) {
  const anchorRef = useRef<Anchor | null>(null);
  const previousItems = useRef<readonly Item[] | null>(null);
  const previousLines = useRef(previewLines);

  /** `getVirtualItemForOffset` binary-searches the measurement table, so this
   *  resolves rows outside the rendered window too. */
  const captureAnchor = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;

    const top = el.scrollTop;
    if (top <= 0) {
      anchorRef.current = null;
      return;
    }

    const row = virtualizer.getVirtualItemForOffset(top);
    anchorRef.current = row
      ? { id: String(row.key), offset: top - row.start }
      : null;
  }, [scrollRef, virtualizer]);

  // Acts only when the item array identity changed, which an idle poll does
  // not do (INV-2). `useVirtualizer` registers its layout effects earlier in
  // the component, so the measurement table is already current here.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    const linesChanged = previousLines.current !== previewLines;
    previousLines.current = previewLines;

    if (previousItems.current === items && !linesChanged) return;
    const hadItems = previousItems.current !== null;
    previousItems.current = items;
    if (!hadItems) return; // first paint: nothing to preserve

    // First on purpose: getTotalSize() recomputes the measurement table, and
    // measurementsCache is only index-aligned with the list once it has run.
    const max = Math.max(0, virtualizer.getTotalSize() - el.clientHeight);

    const anchor = anchorRef.current;
    let desired = el.scrollTop;

    if (anchor) {
      const index = items.findIndex((item) => item.id === anchor.id);
      if (index >= 0) {
        const measurement = virtualizer.measurementsCache[index];
        if (measurement) desired = measurement.start + anchor.offset;
      }
      // index < 0: the anchor row was deleted; the clamp below handles it.
    }

    const next = Math.min(Math.max(desired, 0), max);

    if (Math.abs(next - el.scrollTop) > 0.5) {
      // Writes the DOM scrollTop *and* the virtualizer's tracked offset, so
      // the two cannot disagree for a frame (INV-6).
      virtualizer.scrollToOffset(next, { align: "start", behavior: "auto" });
    }
  });

  return { captureAnchor };
}
