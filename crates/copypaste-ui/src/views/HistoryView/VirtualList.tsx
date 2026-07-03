/**
 * VirtualList — windowed virtual scroll component for HistoryView.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Renders only the rows intersecting the viewport plus an overscan buffer.
 * Row heights are computed from rowHeightFor (supporting mixed image/text
 * heights), stored in a prefix-sum table, and binary-searched for the first
 * visible row — O(log n) per scroll event.
 */
import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
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
  // Read the latest scrollTop from effects without adding it as a dependency
  // (it changes on every scroll frame; see the scroll-anchoring effect below).
  const scrollTopRef = useRef(0);
  scrollTopRef.current = scrollTop;

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

  // CopyPaste-f2ec #17: keep the tracked `scrollTop` (and the real scrollable
  // element) from pointing past the end of a shorter list. `offsets`/`totalH`
  // are correctly recomputed whenever `previewLines` or `imageMaxHeight`
  // change (see the useMemo above), but the DOM's scroll POSITION is only
  // ever updated in response to a user scroll event (`handleScroll` below).
  // If the user is scrolled deep into the list and a display-setting change
  // (or a filter/delete) shrinks `totalH` well below the stale `scrollTop`,
  // `computeVisibleWindow`'s binary search degenerates to a window of just
  // the last row or two — the list visually collapses to "broken/short"
  // until the next manual scroll re-syncs `scrollTop`. Clamping here (and
  // writing the real DOM scrollTop back) whenever the content or viewport
  // height changes keeps the render window valid immediately.
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const maxScrollTop = Math.max(0, totalH - viewportH);
    setScrollTop((prev) => {
      if (prev <= maxScrollTop) return prev;
      el.scrollTop = maxScrollTop;
      return maxScrollTop;
    });
  }, [totalH, viewportH, listRef]);

  // CopyPaste-8ebg.44: scroll anchoring. `items` only receives a new array
  // reference on a genuine content change (useHistoryData's sigRef gate keeps
  // identical polls from producing a new reference; useHistoryFilter's
  // useMemo does the same for filtering/sorting), so this effect fires
  // exactly on the cases the bug report calls out: a background poll
  // prepending a newly-copied item, load-more appending a page, or a
  // delete/undo/pin mutation. Without anchoring, any row-count/height change
  // above the viewport shifts every row below it by the same pixel delta,
  // so the same raw `scrollTop` now points at different content — the
  // viewport visibly jumps out from under a mid-scroll user. We anchor by
  // remembering which item was at the top of the viewport in the OLD list
  // (via the OLD offsets table, captured in the refs below before this
  // render's useMemo overwrote them) and re-deriving a scrollTop that keeps
  // that same item in the same on-screen position in the NEW list.
  const prevItemsRef = useRef(items);
  const prevOffsetsRef = useRef(offsets);
  useEffect(() => {
    if (items !== prevItemsRef.current) {
      const el = listRef.current;
      const oldItems = prevItemsRef.current;
      const oldOffsets = prevOffsetsRef.current;
      const anchorScrollTop = scrollTopRef.current;
      // overscanPx=0 so `start` is the exact top-most row spanning scrollTop.
      const { start: anchorIdx } = computeVisibleWindow(oldOffsets, anchorScrollTop, 0, 0);
      const anchorItem = oldItems[anchorIdx];
      if (el && anchorItem) {
        const withinRow = anchorScrollTop - (oldOffsets[anchorIdx] ?? 0);
        const newIdx = items.findIndex((it) => it.id === anchorItem.id);
        if (newIdx >= 0) {
          const newTop = Math.max(0, (offsets[newIdx] ?? 0) + withinRow);
          if (Math.abs(newTop - anchorScrollTop) > 1) {
            el.scrollTop = newTop;
            setScrollTop(newTop);
          }
        }
        // else: the anchor item is gone (e.g. it was the one just deleted) —
        // fall through and let the browser's natural scroll position stand;
        // clamping above still keeps it in-bounds.
      }
    }
    prevItemsRef.current = items;
    prevOffsetsRef.current = offsets;
  }, [items, offsets, listRef]);

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

  // CopyPaste-8ebg.45: role="list" (not "listbox") deliberately does not carry
  // a real aria-activedescendant — see the g27b.29 note in HistoryRow.tsx for
  // why (role="listbox"/"option" would trip axe's nested-interactive check on
  // the per-row Pin/Preview/Delete buttons). That trade-off means screen
  // readers get no automatic announcement as the roving selection moves via
  // arrow keys. This polite aria-live region closes that gap without
  // reintroducing the nested-interactive violation: it mirrors the active
  // row's own aria-label (set by HistoryRow) whenever the active id changes.
  const [activeAnnouncement, setActiveAnnouncement] = useState("");
  useEffect(() => {
    if (!safeActiveDescendantId) return;
    const el = document.getElementById(safeActiveDescendantId);
    const label = el?.getAttribute("aria-label");
    if (label) setActiveAnnouncement(label);
  }, [safeActiveDescendantId]);

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
      // g27b.29: role="list"/"listitem" (not listbox/option) — the rows carry
      // action <button>s, and a listbox option must not contain focusable
      // descendants (axe nested-interactive); a plain list has no such rule and
      // still satisfies aria-required-children. activedescendant isn't allowed on
      // role=list, so the roving-active id is exposed as a data-* attr instead
      // (the arrow-key handler drives visual selection via onKeyDown regardless).
      role="list"
      aria-label="Clipboard history"
      data-active-descendant={safeActiveDescendantId}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onScroll={handleScroll}
    >
      {/* Spacer establishes the full scroll height; the inner block is offset
          to where the visible window starts. `height` (FUNCTIONAL, per-render
          computed) stays inline; the static `position: relative` moved to the
          `.vlist-spacer` class (g27b.4). */}
      <div className="vlist-spacer" style={{ height: totalH }}>
        {/* §8 selection glide: a single absolutely-positioned layer that animates
            its top/height to the selected row(s). Rendered before the rows so it
            sits behind them; rows carry no selection background of their own. */}
        {glideStyle && (
          <div
            aria-hidden
            className="row-glide"
            // FUNCTIONAL: top/height are per-selection computed placement over
            // the selected row(s); the static position/left/right/z-index/
            // pointer-events moved to the `.row-glide` class (g27b.4).
            style={{
              top: glideStyle.top,
              height: glideStyle.height,
            }}
          />
        )}
        <div className="vlist-window" style={{ top: padTop }}>
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
      {/* CopyPaste-8ebg.45: polite live region announcing the currently active
          row — see the comment above activeAnnouncement for why this exists
          instead of a real aria-activedescendant. */}
      <span className="sr-only" aria-live="polite" aria-atomic="true">
        {activeAnnouncement}
      </span>
    </div>
  );
}
