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
        position: "absolute",
        left: 0,
        right: 0,
        top,
        height,
        // §3 selection fill — token so it re-themes in light mode.
        background: "var(--ide-selection)",
        // zuzu: suppress the 130ms transition during scroll so the glide layer
        // doesn't visibly slide out of the container during momentum scroll.
        // Also hide (opacity 0) if the selected item is outside the viewport.
        transition: prefersReduced.current || isScrolling
          ? "none"
          : "top 130ms cubic-bezier(0.2,0,0,1), height 130ms cubic-bezier(0.2,0,0,1)",
        opacity: visible && !isScrolling ? 1 : 0,
        // Behind row content (z=0), above list background.
        zIndex: 0,
        pointerEvents: "none",
      }}
    />
  );
}
