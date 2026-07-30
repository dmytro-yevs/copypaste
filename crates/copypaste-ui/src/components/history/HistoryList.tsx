/**
 * INV-8: `role="list"`/`listitem`, never listbox/option — the rows carry
 * buttons, and option is `childrenPresentational` (axe `nested-interactive`).
 * INV-9/A11Y-14: the announcer is a *sibling* of the list; inside it, it fails
 * `aria-required-children` (CopyPaste-wrfn).
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

import { Button } from "@/components/ui/button";
import { useScrollAnchor } from "@/hooks/useScrollAnchor";
import { useTranslation } from "@/i18n";
import type { Item } from "@/lib/ipc";
import {
  COPY_FLASH_MS,
  LOAD_MORE_THRESHOLD_PX,
  overscanRows,
  rowHeight,
} from "@/lib/layout";
import { HistoryRow, rowLabel } from "@/components/history/HistoryRow";
import type { Selection } from "@/hooks/useSelection";

/** ⌘1–⌘9 only; there is no ⌘0 and no second row of ten. */
export const QUICK_SLOTS = 9;

/**
 * Which row a quick-copy digit means, or `null`.
 *
 * Inactive while a search is running (manifest §3.5.3): the digits address
 * positions in the history, and a filtered list renumbers them under the
 * user's fingers between keystrokes.
 */
export function quickSlot(key: string, searching: boolean): number | null {
  if (searching) return null;
  const digit = Number.parseInt(key, 10);
  if (!Number.isInteger(digit) || digit < 1 || digit > QUICK_SLOTS) return null;
  return digit - 1;
}

interface HistoryListProps {
  items: readonly Item[];
  activeId: string | null;
  onActiveIdChange: (id: string | null) => void;
  revealedId: string | null;
  revealedContent: string | null;
  revealPendingId: string | null;
  previewLines: number;
  searching: boolean;
  selection: Selection;
  hasMore: boolean;
  loadingMore: boolean;
  onReveal: (item: Item) => void;
  onHide: () => void;
  onCopy: (item: Item) => void;
  /** Copy and then dismiss the window — the ⌘1–⌘9 path. */
  onQuickCopy: (item: Item) => void;
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
  searching,
  selection,
  hasMore,
  loadingMore,
  onReveal,
  onHide,
  onCopy,
  onQuickCopy,
  onTogglePin,
  onDelete,
  onLoadMore,
  listRef,
}: HistoryListProps) {
  const { t } = useTranslation();
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

  // Keyed on activeId alone: the age ticking over in a row's label must not
  // re-announce a selection that did not change (INV-9).
  useEffect(() => {
    const item = itemsRef.current.find((candidate) => candidate.id === activeId);
    setAnnouncement(item ? rowLabel(item) : "");
  }, [activeId]);

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
    // From the height model, not scrollIntoView: the target row may not be in
    // the DOM at all.
    virtualizer.scrollToIndex(index, { align: "auto" });
  }

  function onScroll(event: UIEvent<HTMLDivElement>) {
    captureAnchor();
    const el = event.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < LOAD_MORE_THRESHOLD_PX) {
      onLoadMore();
    }
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (items.length === 0) return;

    // ⌘1–⌘9: the fastest path in a clipboard manager — hotkey, glance, ⌘3.
    // Checked before the switch because the digit keys carry no other meaning
    // here and the modifier makes them unambiguous.
    if (event.metaKey || event.ctrlKey) {
      if (event.key.toLowerCase() === "a") {
        event.preventDefault();
        selection.selectAll();
        return;
      }
      const slot = quickSlot(event.key, searching);
      if (slot !== null) {
        const item = items[slot];
        if (item) {
          event.preventDefault();
          setFlashId(item.id);
          onQuickCopy(item);
        }
        return;
      }
    }

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
      case " ":
        // Space toggles the checkbox, but only in selection mode — otherwise
        // it is the browser's page-down and taking that away breaks scrolling
        // for a keyboard user.
        if (selection.selecting && item) {
          event.preventDefault();
          selection.toggle(item.id);
        }
        break;
      case "Enter":
        if (item) {
          event.preventDefault();
          if (selection.selecting) selection.toggle(item.id);
          else copy(item);
        }
        break;
      case "Backspace":
      case "Delete": {
        if (item && !selection.selecting) {
          event.preventDefault();
          // Moves before removal, so the selection never dangles on the id
          // that is about to disappear.
          const next = items[index + 1] ?? items[index - 1] ?? null;
          onActiveIdChange(next ? next.id : null);
          onDelete(item);
        }
        break;
      }
      case "Escape":
        // Leaves selection mode first (§3.1.4): Escape clears the multi-select
        // if there is one, and only then the single selection.
        if (selection.selecting) selection.end();
        else onActiveIdChange(null);
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
        className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-s-2 pb-s-2 outline-none focus-visible:ring-[3px] focus-visible:ring-inset focus-visible:ring-ring"
      >
        <div
          role="list"
          aria-label={t("history.list.label")}
          aria-multiselectable={selection.selecting || undefined}
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
                // The name lives on the row's own button; naming the wrapper
                // too would announce every row twice.
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
                  selecting={selection.selecting}
                  checked={selection.selected.has(item.id)}
                  onToggleChecked={(clicked) => selection.toggle(clicked.id)}
                  onSelect={(clicked) => onActiveIdChange(clicked.id)}
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

        {/* An explicit control, not only the near-bottom trigger. A filtered
            list can be three rows long with a thousand unpaged matches behind
            it, and three rows do not scroll — so scrolling alone would strand
            the user on an empty result that is not empty. */}
        {hasMore && (
          <div className="flex justify-center py-s-3">
            <Button
              variant="outline"
              size="sm"
              disabled={loadingMore}
              onClick={onLoadMore}
            >
              {t(
                loadingMore ? "history.list.loadingMore" : "history.list.loadMore",
              )}
            </Button>
          </div>
        )}
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
