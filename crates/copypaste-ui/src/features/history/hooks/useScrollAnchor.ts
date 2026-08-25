/**
 * INV-1 (content-anchored scrolling) and INV-6 (the shrink clamp). TanStack
 * Virtual does not preserve either for a mutating list, so the arithmetic is
 * owned here. The anchor is item id plus intra-row offset, never an index.
 * At `scrollTop === 0` nothing is anchored, so prepends remain visible.
 */
import { type RefObject, useCallback, useLayoutEffect, useRef } from "react";
import type { Virtualizer } from "@tanstack/react-virtual";

import type { Item } from "@/lib/ipc";

interface Anchor {
  readonly id: string;
  /** Distance from the top of the anchored row to the viewport top. */
  readonly offset: number;
}

interface Options {
  scrollRef: RefObject<HTMLDivElement | null>;
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  items: readonly Item[];
  /** `null` marks a non-item virtual row such as a device heading. */
  anchorIds?: readonly (string | null)[];
  /** Triggers INV-6 immediately when a preview-line change shrinks rows. */
  previewLines: number;
}

export function useScrollAnchor({
  scrollRef,
  virtualizer,
  items,
  anchorIds,
  previewLines,
}: Options) {
  const anchorRef = useRef<Anchor | null>(null);
  const previousItems = useRef<readonly Item[] | null>(null);
  const previousLines = useRef(previewLines);

  /** Resolves rows outside the rendered window from the measurement table. */
  const captureAnchor = useCallback(() => {
    const element = scrollRef.current;
    if (!element) return;

    const top = element.scrollTop;
    if (top <= 0) {
      anchorRef.current = null;
      return;
    }

    const row = virtualizer.getVirtualItemForOffset(top);
    if (!row) {
      anchorRef.current = null;
      return;
    }

    const ids = anchorIds ?? items.map((item) => item.id);
    let index = row.index;
    while (index < ids.length && ids[index] === null) index += 1;
    const id = ids[index];
    const measurement = virtualizer.measurementsCache[index];
    anchorRef.current =
      id && measurement ? { id, offset: top - measurement.start } : null;
  }, [anchorIds, items, scrollRef, virtualizer]);

  // An idle poll preserves the items array identity and does no scroll work.
  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element) return;

    const linesChanged = previousLines.current !== previewLines;
    previousLines.current = previewLines;
    if (previousItems.current === items && !linesChanged) return;

    const hadItems = previousItems.current !== null;
    previousItems.current = items;
    if (!hadItems) return;

    // getTotalSize updates the measurement table before it is indexed below.
    const max = Math.max(
      0,
      virtualizer.getTotalSize() - element.clientHeight,
    );
    const anchor = anchorRef.current;
    let desired = element.scrollTop;

    if (anchor) {
      const ids = anchorIds ?? items.map((item) => item.id);
      const index = ids.indexOf(anchor.id);
      if (index >= 0) {
        const measurement = virtualizer.measurementsCache[index];
        if (measurement) desired = measurement.start + anchor.offset;
      }
      // A deleted anchor falls through to the shrink clamp.
    }

    const next = Math.min(Math.max(desired, 0), max);
    if (Math.abs(next - element.scrollTop) > 0.5) {
      // Keeps the DOM position and virtualizer offset in the same frame.
      virtualizer.scrollToOffset(next, {
        align: "start",
        behavior: "auto",
      });
    }
  });

  return { captureAnchor };
}
