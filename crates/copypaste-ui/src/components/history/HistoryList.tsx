/**
 * The virtualised history list.
 *
 * Invariants implemented here:
 *
 *  INV-1/6  scroll anchoring and the shrink clamp — see useScrollAnchor
 *  INV-4    load-more merges: near the bottom, ask for the next page; the
 *           query keeps every loaded page, so a poll cannot replace them
 *  INV-5    every row reserves the full preview-line cap; the estimate is
 *           never derived from the content (see lib/layout)
 *  INV-7    the active-descendant pointer only ever names a mounted row
 *  INV-8    role="list" / role="listitem", never listbox/option — the rows
 *           carry buttons, and option is childrenPresentational
 *  INV-9    a visually-hidden polite live region, a **sibling** of the list
 *           (a live region inside role="list" fails aria-required-children)
 *  INV-32   selection is tracked by item id and re-resolved to an index every
 *           render; a poll can reorder between keydown and Enter
 *
 * Keyboard (manifest §3.1.4): Up/Down move without wrapping, Enter copies,
 * Backspace/Delete deletes (moving the selection on first), Escape clears.
 */
import {
  type KeyboardEvent,
  type RefObject,
  type UIEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { useScrollAnchor } from "@/hooks/useScrollAnchor";
import type { Item } from "@/lib/ipc";
import {
  COPY_FLASH_MS,
  LOAD_MORE_THRESHOLD_PX,
  overscanRows,
  rowHeight,
} from "@/lib/layout";
import { HistoryRow, rowLabel } from "@/components/history/HistoryRow";

interface HistoryListProps {
  items: readonly Item[];
  activeId: string | null;
  onActiveIdChange: (id: string | null) => void;
  revealedId: string | null;
  revealedContent: string | null;
  revealPendingId: string | null;
  previewLines: number;
  onReveal: (item: Item) => void;
  onHide: () => void;
  onCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onLoadMore: () => void;
  listRef: RefObject<HTMLDivElement | null>;
}

export function HistoryList({
  items,
  activeId,
  onActiveIdChange,
  revealedId,
  revealedContent,
  revealPendingId,
  previewLines,
  onReveal,
  onHide,
  onCopy,
  onTogglePin,
  onDelete,
  onLoadMore,
  listRef,
}: HistoryListProps) {
  const [announcement, setAnnouncement] = useState("");
  const [flashId, setFlashId] = useState<string | null>(null);
  const itemsRef = useRef(items);
  itemsRef.current = items;

  // INV-5: the reservation is a function of the *setting*, never of the item.
  const estimateSize = useCallback(() => rowHeight(previewLines), [previewLines]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => listRef.current,
    estimateSize,
    getItemKey: (index) => items[index]?.id ?? index,
    overscan: overscanRows(previewLines),
  });

  const { captureAnchor } = useScrollAnchor({
    scrollRef: listRef,
    virtualizer,
    items,
    previewLines,
  });

  const virtualRows = virtualizer.getVirtualItems();

  // INV-9 / A11Y-2. Keyed on activeId alone so the age ticking over in a row's
  // label can never re-announce a selection that did not change.
  useEffect(() => {
    const item = itemsRef.current.find((candidate) => candidate.id === activeId);
    setAnnouncement(item ? rowLabel(item) : "");
  }, [activeId]);

  // Copy flash, manifest §3.1.5.
  useEffect(() => {
    if (flashId === null) return;
    const timer = setTimeout(() => setFlashId(null), COPY_FLASH_MS);
    return () => clearTimeout(timer);
  }, [flashId]);

  function copy(item: Item) {
    setFlashId(item.id);
    onCopy(item);
  }

  function select(index: number) {
    const item = items[index];
    if (!item) return;
    onActiveIdChange(item.id);
    // Scrolling comes from the height model, not scrollIntoView: the target row
    // may not be in the DOM at all (manifest §3.1.4).
    virtualizer.scrollToIndex(index, { align: "auto" });
  }

  function onScroll(event: UIEvent<HTMLDivElement>) {
    captureAnchor();
    const el = event.currentTarget;
    // §3.1.2: fire load-more within 300px of the bottom. The guards (ready,
    // not already fetching, more to fetch) live in the caller's query.
    if (el.scrollHeight - el.scrollTop - el.clientHeight < LOAD_MORE_THRESHOLD_PX) {
      onLoadMore();
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (items.length === 0) return;

    // INV-32: resolve the index from the id, every time.
    const index = activeId
      ? items.findIndex((item) => item.id === activeId)
      : -1;
    const item = index >= 0 ? items[index] : undefined;

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        // Clamped at both ends; the history list does not wrap (AT-10).
        select(index < 0 ? 0 : Math.min(index + 1, items.length - 1));
        break;
      case "ArrowUp":
        event.preventDefault();
        select(index < 0 ? items.length - 1 : Math.max(index - 1, 0));
        break;
      case "Home":
        event.preventDefault();
        select(0);
        break;
      case "End":
        event.preventDefault();
        select(items.length - 1);
        break;
      case "Enter":
        if (item) {
          event.preventDefault();
          copy(item);
        }
        break;
      case "Backspace":
      case "Delete": {
        if (item) {
          event.preventDefault();
          // Selection moves to the next row *before* removal (§3.1.4), so it
          // never dangles on the id that is about to disappear.
          const next = items[index + 1] ?? items[index - 1] ?? null;
          onActiveIdChange(next ? next.id : null);
          onDelete(item);
        }
        break;
      }
      case "Escape":
        onActiveIdChange(null);
        break;
      default:
        break;
    }
  }

  // INV-7 / AT-11: never point at a row outside the rendered window.
  const activeRendered =
    activeId !== null && virtualRows.some((row) => String(row.key) === activeId);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={listRef}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onScroll={onScroll}
        className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-s-2 pb-s-2 outline-none focus-visible:ring-[3px] focus-visible:ring-inset focus-visible:ring-ring/50"
      >
        <div
          role="list"
          aria-label="Clipboard history"
          data-active-descendant={
            activeRendered ? `history-row-${activeId}` : undefined
          }
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualRows.map((row) => {
            const item = items[row.index];
            if (!item) return null;
            return (
              <div
                key={row.key}
                id={`history-row-${item.id}`}
                role="listitem"
                aria-label={rowLabel(item)}
                aria-current={item.id === activeId ? "true" : undefined}
                className="absolute top-0 left-0 w-full"
                style={{
                  height: row.size,
                  transform: `translateY(${row.start}px)`,
                }}
              >
                <HistoryRow
                  item={item}
                  active={item.id === activeId}
                  flashing={item.id === flashId}
                  revealedContent={
                    item.id === revealedId ? revealedContent : null
                  }
                  revealPending={item.id === revealPendingId}
                  previewLines={previewLines}
                  onActivate={(clicked) => {
                    onActiveIdChange(clicked.id);
                    listRef.current?.focus();
                    copy(clicked);
                  }}
                  onCopy={copy}
                  onTogglePin={onTogglePin}
                  onDelete={onDelete}
                  onReveal={onReveal}
                  onHide={onHide}
                />
              </div>
            );
          })}
        </div>
      </div>

      {/* INV-9 / A11Y-14: a sibling of the list, never a child of it. */}
      <div
        aria-live="polite"
        aria-atomic="true"
        className="sr-only"
        data-testid="history-announcer"
      >
        {announcement}
      </div>
    </div>
  );
}
