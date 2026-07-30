/**
 * INV-1 (content-anchored scrolling) and INV-6 (shrink clamp).
 *
 * Manifest 06 flags this as the highest-risk regression in the UI rewrite, and
 * §9.1 is explicit that TanStack Virtual does **not** give it to you: the
 * library only anchors by key when `anchorTo: "end"` is set (the chat-style
 * stick-to-bottom case). A history list anchors to the start, so this is ours
 * to implement.
 *
 * The contract:
 *
 *   - The anchor is **item id + intra-row offset**, never a pixel offset and
 *     never an index — a poll can reorder or prepend between two frames.
 *   - When the list mutates (a poll prepends, a pin re-sorts, a delete removes
 *     a row), the anchored row stays under the same point on screen.
 *   - If the anchor row is gone — it was the one deleted (AT-3) — fall through
 *     to the natural position and clamp; never throw.
 *   - When the content shrinks below the current scroll offset, both the
 *     virtualizer's tracked offset and the real DOM `scrollTop` clamp
 *     immediately, not on the next user scroll (INV-6, AT-4).
 *
 * The one deliberate exception: at `scrollTop === 0` the view is pinned to the
 * top, so a prepend reveals the new item instead of pushing the old first row
 * back under the cursor. Anchoring an already-top-most view would be a visible
 * jump, which is the thing INV-1 exists to prevent.
 */
import { type RefObject, useCallback, useLayoutEffect, useRef } from "react";
import type { Virtualizer } from "@tanstack/react-virtual";

import type { Item } from "../lib/ipc";

interface Anchor {
  readonly id: string;
  /** Distance from the top of the anchored row to the top of the viewport. */
  readonly offset: number;
}

interface Options {
  scrollRef: RefObject<HTMLDivElement | null>;
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  items: readonly Item[];
}

export function useScrollAnchor({ scrollRef, virtualizer, items }: Options) {
  const anchorRef = useRef<Anchor | null>(null);
  const previousItems = useRef<readonly Item[] | null>(null);

  /**
   * Record which row the user is looking at. Called on every scroll event, so
   * the anchor is always current with respect to the last thing the user did.
   * `getVirtualItemForOffset` binary-searches the measurement table, so it
   * resolves rows outside the rendered window too.
   */
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

  // Runs after every render, but only acts when the item array identity
  // actually changed — which, thanks to React Query's structural sharing
  // (INV-2), an idle poll does not do. `useVirtualizer` registers its own
  // layout effects earlier in this component, so by the time this runs the
  // measurement table already reflects the new list.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    if (previousItems.current === items) return;
    const hadItems = previousItems.current !== null;
    previousItems.current = items;
    if (!hadItems) return; // first paint: nothing to preserve

    const anchor = anchorRef.current;
    let desired = el.scrollTop;

    if (anchor) {
      const index = items.findIndex((item) => item.id === anchor.id);
      if (index >= 0) {
        // measurementsCache is index-aligned with the current item list.
        const measurement = virtualizer.measurementsCache[index];
        if (measurement) desired = measurement.start + anchor.offset;
      }
      // index < 0: the anchor row was deleted. Keep the natural offset and let
      // the clamp below deal with it.
    }

    const max = Math.max(0, virtualizer.getTotalSize() - el.clientHeight);
    const next = Math.min(Math.max(desired, 0), max);

    if (Math.abs(next - el.scrollTop) > 0.5) {
      // scrollToOffset writes the DOM scrollTop *and* the virtualizer's own
      // tracked offset, so the two cannot disagree for a frame (INV-6).
      virtualizer.scrollToOffset(next, { align: "start", behavior: "auto" });
    }
  });

  return { captureAnchor };
}
