// ── GlideHighlight ────────────────────────────────────────────────────────────
// §4/§8: A single absolutely-positioned layer that slides to the selected row.
// Animates top + height using the shared motion tokens (--dur/--ease from
// tokens.css). Instant when prefers-reduced-motion is set (--dur collapses to
// 0ms in that case too, so the explicit isScrolling/prefersReduced guard below
// is a belt-and-suspenders skip of the transition altogether).
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { HistoryEntry } from "../lib/ipc";

export interface GlideHighlightProps {
  selectedIdx: number;
  items: Array<{ item: HistoryEntry; positions: number[] }>;
  textRowHeight: number;
  imageMaxHeight: number;
  listRef: React.RefObject<HTMLUListElement | null>;
  /** zuzu: when true, suppress CSS transition and hide the layer if the
   *  selected item has scrolled out of the visible clip region. */
  isScrolling?: boolean;
}

export function GlideHighlight({
  selectedIdx,
  items,
  textRowHeight,
  imageMaxHeight,
  listRef,
  isScrolling = false,
}: GlideHighlightProps) {
  const [top, setTop] = useState(0);
  const [height, setHeight] = useState(textRowHeight);
  // zuzu: track visibility separately so we can hide when out-of-viewport.
  const [visible, setVisible] = useState(true);
  // Track whether the user prefers reduced motion so we can skip animation.
  // Guard against jsdom / test environments where matchMedia is unavailable.
  const prefersReduced = useRef(
    typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );

  // CopyPaste-8ebg.64: this was a plain useEffect, which fires AFTER the
  // browser has already painted the initial top/height state (0 /
  // textRowHeight) — visible as one frame of wrong geometry (e.g. the
  // highlight briefly covering row 0 before snapping to the real selected
  // row) every time the popup mounts or the list re-renders with new items.
  // useLayoutEffect runs synchronously after DOM mutations but before paint,
  // so the corrected top/height are committed in the same frame.
  useLayoutEffect(() => {
    // Read geometry directly from the rendered list item so we match the
    // exact heights produced by popupRowHeight() without duplicating the calc.
    const list = listRef.current;
    if (!list) return;
    const child = list.children[selectedIdx] as HTMLElement | undefined;
    if (!child) return;
    // offsetTop is relative to the list's offsetParent (which is our wrapper div).
    // We also need to offset by the list's own scrollTop so the glide layer
    // tracks the visible position (list scrolls, wrapper does not).
    const newTop = child.offsetTop - list.scrollTop;
    setTop(newTop);
    setHeight(child.offsetHeight);
    // zuzu: hide when the selected item has scrolled completely out of view.
    setVisible(newTop >= 0 && newTop < list.clientHeight);
  }, [selectedIdx, items, textRowHeight, imageMaxHeight, listRef]);

  // Keep glide in sync when the list scrolls (user scrolls with keyboard nav).
  // zuzu: also update visibility and freeze transition during scroll.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const onScroll = () => {
      const child = list.children[selectedIdx] as HTMLElement | undefined;
      if (!child) return;
      const newTop = child.offsetTop - list.scrollTop;
      setTop(newTop);
      // Hide if scrolled out of the visible clip region.
      setVisible(newTop >= 0 && newTop < list.clientHeight);
    };
    list.addEventListener("scroll", onScroll, { passive: true });
    return () => list.removeEventListener("scroll", onScroll);
  }, [selectedIdx, listRef]);

  return (
    <div
      aria-hidden
      // g27b.4: static position/left/right/z-index/pointer-events moved to
      // the shared `.row-glide` class (patterns.css) — VirtualList.tsx's
      // History glide layer uses the same class. `top`/`height` (per-row
      // measured geometry) and `opacity`/`transition` (motion state) stay
      // inline below since they're genuinely per-render computed values.
      className="row-glide"
      style={{
        // Functional measurement/positioning — tracks the selected row's
        // geometry so the glide layer overlays the right list item.
        top,
        height,
        // Functional: suppress the transition during scroll, and hide
        // (opacity 0) when the selected item has scrolled out of view —
        // this is glide/visibility behavior wired to preserved state
        // (visible/isScrolling/prefersReduced), not decoration.
        // Durations/easing come from the motion tokens (tokens.css --dur/--ease)
        // so this auto-no-ops under prefers-reduced-motion (--dur collapses to 0ms).
        transition: prefersReduced.current || isScrolling
          ? "none"
          : "top var(--dur) var(--ease), height var(--dur) var(--ease)",
        opacity: visible && !isScrolling ? 1 : 0,
      }}
    />
  );
}
