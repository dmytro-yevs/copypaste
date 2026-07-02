/**
 * VirtualList — windowed virtual scroll component for HistoryView.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Renders only the rows intersecting the viewport plus an overscan buffer.
 * Row heights are computed from rowHeightFor (supporting mixed image/text
 * heights), stored in a prefix-sum table, and binary-searched for the first
 * visible row — O(log n) per scroll event.
 */
import React, { useState, useEffect, useMemo, useCallback } from "react";
import type { HistoryEntry } from "../../lib/ipc";
import {
  rowHeightFor,
  buildOffsets,
  computeVisibleWindow,
} from "./historyVirtualizer";

/** How many px from the bottom of the scroll container triggers load-more. */
export const LOAD_MORE_THRESHOLD_PX = 300;

export interface VirtualListProps {
  items: HistoryEntry[];
  previewSize: number;
  imageMaxHeight: number;
  density: "comfortable" | "compact" | "spacious";
  /** Preview-lines setting — grows text-row height so multi-line clamp fits. */
  previewLines: number;
  listRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
  /**
   * Render a single row. `visibleIndex` is the row's 0-based position within
   * the currently-rendered visible window (not the full list index) — used by
   * the parent to compute mount-stagger delays.
   */
  renderRow: (entry: HistoryEntry, visibleIndex: number) => React.ReactNode;
  /**
   * Called when the user scrolls to within LOAD_MORE_THRESHOLD_PX of the
   * bottom of the list. The parent uses this to fetch the next page.
   * Optional — omit when load-more is not needed.
   */
  onNearBottom?: () => void;
  /** ID of the currently keyboard-selected option — drives aria-activedescendant. */
  activeDescendantId?: string | null;
  /**
   * §8 selection glide: absolute top/height (in list-content px) of the layer
   * that animates to the selected row(s). `null` hides the layer. Rows carry no
   * selection background themselves, so this is the sole selection indicator.
   */
  glideStyle?: { top: number; height: number } | null;
  /**
   * CopyPaste redesign (Slice 3, 3.5): class name(s) applied to the scrollable
   * list container (the root role="listbox" div below). The caller composes
   * "list" / "list selecting" from its own selectionMode state — VirtualList
   * only forwards it onto the DOM node it owns.
   */
  className?: string;
}

export function VirtualList({
  items,
  previewSize,
  imageMaxHeight,
  density,
  previewLines,
  listRef,
  onKeyDown,
  renderRow,
  onNearBottom,
  activeDescendantId,
  glideStyle,
  className,
}: VirtualListProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(0);

  // Prefix-sum offsets: offsets[i] is the top of row i; offsets[n] is total height.
  // Memoized on item count/ids and display settings so scroll events (which only
  // update scrollTop state) do NOT rebuild the full height table on every frame.
  // Only recomputed when the item list, heights, or density actually change.
  const offsets = useMemo(
    () => buildOffsets(items.map((it) => rowHeightFor(it, previewSize, imageMaxHeight, density, previewLines))),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      // Stable reference identity: items array changes only when content changes.
      items,
      previewSize,
      imageMaxHeight,
      density,
      previewLines,
    ]
  );
  const totalH = offsets[items.length] ?? 0;

  // Measure the viewport height and keep it current on resize.
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    setViewportH(el.clientHeight);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setViewportH(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, [listRef]);

  const { start, end } = computeVisibleWindow(offsets, scrollTop, viewportH);
  const visible = items.slice(start, end);
  const padTop = offsets[start] ?? 0;

  // CopyPaste-5917.33: aria-activedescendant must only reference an element that
  // is actually present in the DOM. The virtual window only renders rows in the
  // viewport ±overscan; if the active row has scrolled outside that window its
  // DOM id does not exist and screen readers report an invalid reference.
  // Derive whether the active row falls within [start, end) and clear the
  // attribute when it is off-screen. Note: the scroll-into-view useEffect in
  // HistoryView ensures the selected row is scrolled into view on keyboard nav,
  // so in practice the row will be rendered shortly after selection — clearing
  // here is a safety net for the brief window before the scroll resolves.
  const activeIdInView = activeDescendantId
    ? visible.some((it) => `clip-${it.id}` === activeDescendantId)
    : false;
  // Coerce null → undefined: the DOM aria-activedescendant prop accepts
  // string | undefined, not null (activeDescendantId may be null).
  const safeActiveDescendantId = activeIdInView
    ? (activeDescendantId ?? undefined)
    : undefined;

  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const el = e.target as HTMLDivElement;
      setScrollTop(el.scrollTop);
      // Fire onNearBottom when the user is within the threshold of the bottom.
      // scrollHeight - scrollTop - clientHeight gives the remaining distance.
      if (onNearBottom !== undefined) {
        const remaining = el.scrollHeight - el.scrollTop - el.clientHeight;
        if (remaining < LOAD_MORE_THRESHOLD_PX) {
          onNearBottom();
        }
      }
    },
    [onNearBottom]
  );

  return (
    <div
      ref={listRef}
      // .scroll-y makes this the bounded scroll container (flex:1; min-height:0;
      // overflow-y:auto) so the virtualizer's clientHeight/scrollTop math holds.
      className={`${className} scroll-y`}
      role="listbox"
      aria-label="Clipboard history"
      aria-activedescendant={safeActiveDescendantId}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onScroll={handleScroll}
    >
      {/* Spacer establishes the full scroll height; the inner block is offset
          to where the visible window starts. FUNCTIONAL: drives the virtual
          scroller's total scrollable height. */}
      <div style={{ height: totalH, position: "relative" }}>
        {/* §8 selection glide: a single absolutely-positioned layer that animates
            its top/height to the selected row(s). Rendered before the rows so it
            sits behind them; rows carry no selection background of their own. */}
        {glideStyle && (
          <div
            aria-hidden
            // FUNCTIONAL: position/left/right/top/height/pointerEvents drive the
            // glide layer's computed placement over the selected row(s) and
            // ensure it never blocks clicks on the rows beneath it.
            style={{
              position: "absolute",
              left: 0,
              right: 0,
              top: glideStyle.top,
              height: glideStyle.height,
              pointerEvents: "none",
            }}
          />
        )}
        <div style={{ position: "absolute", top: padTop, left: 0, right: 0 }}>
          {/* Wrap each row in a keyed fragment so React tracks identity by item
              id across the sliding virtual window — not by position within the
              visible slice, which changes on every scroll. The renderRow callback
              also sets key on HistoryRow (belt-and-suspenders), but the key here
              at the map() call site is what React actually uses for reconciliation.
              `start + i` is the row's absolute index, used for mount-stagger delay. */}
          {visible.map((entry, i) => (
            <React.Fragment key={entry.id}>
              {renderRow(entry, start + i)}
            </React.Fragment>
          ))}
        </div>
      </div>
    </div>
  );
}
