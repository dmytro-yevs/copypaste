// ── GlideHighlight ────────────────────────────────────────────────────────────
// §4/§8: A single absolutely-positioned layer that slides to the selected row.
// Animates top + height over 130ms ease (CSS var --ease-standard if defined,
// else cubic-bezier(0.2,0,0,1)). Instant when prefers-reduced-motion is set.
import React, { useEffect, useRef, useState } from "react";
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

  useEffect(() => {
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
      style={{
        // Functional measurement/positioning only — tracks the selected row's
        // geometry so the glide layer overlays the right list item.
        position: "absolute",
        left: 0,
        right: 0,
        top,
        height,
        // Functional: suppress the transition during scroll, and hide
        // (opacity 0) when the selected item has scrolled out of view —
        // this is glide/visibility behavior wired to preserved state
        // (visible/isScrolling/prefersReduced), not decoration.
        transition: prefersReduced.current || isScrolling
          ? "none"
          : "top 130ms cubic-bezier(0.2,0,0,1), height 130ms cubic-bezier(0.2,0,0,1)",
        opacity: visible && !isScrolling ? 1 : 0,
        // Functional: absolutely-positioned elements paint above static
        // siblings by default regardless of DOM order — zIndex 0 here keeps
        // this layer behind the row content (text must stay legible).
        zIndex: 0,
        pointerEvents: "none",
      }}
    />
  );
}
