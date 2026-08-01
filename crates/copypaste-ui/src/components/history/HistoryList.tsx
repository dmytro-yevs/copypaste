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
  useMemo,
  useRef,
  useState,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useScrollAnchor } from "@/hooks/useScrollAnchor";
import { useTranslation } from "@/i18n";
import type { Item } from "@/lib/ipc";
import { isAndroidPlatform } from "@/lib/platform";
import {
  COPY_FLASH_MS,
  imageRowHeight,
  LOAD_MORE_THRESHOLD_PX,
  overscanRows,
  rowHeight,
} from "@/lib/layout";
import { HistoryRow, rowLabel } from "@/components/history/HistoryRow";
import {
  markedOrigins,
  originLabel,
  originName,
  originOf,
} from "@/components/history/origin";
import type { Selection } from "@/hooks/useSelection";

/** ⌘1–⌘9 only; there is no ⌘0 and no second row of ten. */
export const QUICK_SLOTS = 9;
const GROUP_HEADER_PX = 32;

/** Inactive while a search is running (manifest §3.5.3): the digits address
 *  positions in the history, and a filtered list renumbers them under the
 *  user's fingers between keystrokes. */
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
  groupedByDevice: boolean;
  selection: Selection;
  hasMore: boolean;
  loadingMore: boolean;
  onReveal: (item: Item) => void;
  onCopy: (item: Item) => void;
  /** Copy and then dismiss the window — the ⌘1–⌘9 path. */
  onQuickCopy: (item: Item) => void;
  onTogglePin: (item: Item) => void;
  onDelete: (item: Item) => void;
  onOpen: (item: Item) => void;
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
  groupedByDevice,
  selection,
  hasMore,
  loadingMore,
  onReveal,
  onCopy,
  onQuickCopy,
  onTogglePin,
  onDelete,
  onOpen,
  onLoadMore,
  listRef,
}: HistoryListProps) {
  const { t } = useTranslation();
  const [announcement, setAnnouncement] = useState("");
  const [flashId, setFlashId] = useState<string | null>(null);
  const itemsRef = useRef(items);
  itemsRef.current = items;

  const marked = useMemo(() => markedOrigins(items), [items]);
  const markedRef = useRef(marked);
  markedRef.current = marked;

  const entries = useMemo(() => {
    const rows: Array<
      | { readonly type: "item"; readonly itemIndex: number }
      | {
          readonly type: "group";
          readonly key: string;
          readonly label: string;
        }
    > = [];
    let previousOrigin: string | null = null;
    items.forEach((item, itemIndex) => {
      const origin = originOf(item);
      const id = origin?.id ?? "";
      if (groupedByDevice && id !== previousOrigin) {
        rows.push({
          type: "group",
          key: `history-device-${id || "unknown"}`,
          label: origin ? originName(origin) : t("history.list.unknownDevice"),
        });
      }
      rows.push({ type: "item", itemIndex });
      previousOrigin = id;
    });
    return rows;
  }, [groupedByDevice, items, t]);

  const anchorIds = useMemo(
    () =>
      entries.map((entry) =>
        entry.type === "item" ? (items[entry.itemIndex]?.id ?? null) : null,
      ),
    [entries, items],
  );

  const estimateSize = useCallback(
    (index: number) =>
      entries[index]?.type === "group"
        ? GROUP_HEADER_PX
        : items[entries[index]?.itemIndex ?? -1]?.content_type.toLowerCase().startsWith("image/")
          ? imageRowHeight(previewLines)
          : rowHeight(previewLines),
    [entries, items, previewLines],
  );

  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => listRef.current,
    estimateSize,
    getItemKey: (index) => {
      const entry = entries[index];
      return entry?.type === "group"
        ? entry.key
        : (items[entry?.itemIndex ?? -1]?.id ?? index);
    },
    overscan: overscanRows(previewLines),
  });

  const { captureAnchor } = useScrollAnchor({
    scrollRef: listRef,
    virtualizer,
    items,
    anchorIds,
    previewLines,
  });

  const virtualRows = virtualizer.getVirtualItems();

  // Keyed on activeId alone: the age ticking over in a row's label must not
  // re-announce a selection that did not change (INV-9).
  useEffect(() => {
    const item = itemsRef.current.find((candidate) => candidate.id === activeId);
    setAnnouncement(item ? rowLabel(item, originLabel(item, markedRef.current)) : "");
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
    const virtualIndex = entries.findIndex(
      (entry) => entry.type === "item" && entry.itemIndex === index,
    );
    if (virtualIndex >= 0) {
      virtualizer.scrollToIndex(virtualIndex, { align: "auto" });
    }
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

    // Before the switch: with the modifier held the digits are unambiguous.
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
      case "ArrowRight":
        // The only keyboard route into the detail view that does not require
        // tabbing through the row's other actions first.
        if (item) {
          event.preventDefault();
          onOpen(item);
        }
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
        role="list"
        aria-label={t("history.list.label")}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onScroll={onScroll}
        data-active-descendant={
          activeRendered ? `history-row-${activeId}` : undefined
        }
        className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-s-2 pb-s-2 outline-none focus-visible:ring-[3px] focus-visible:ring-inset focus-visible:ring-ring"
      >
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualRows.map((row) => {
            const entry = entries[row.index];
            if (!entry) return null;
            if (entry.type === "group") {
              return (
                <div
                  key={row.key}
                  role="listitem"
                  className="absolute top-0 left-0 flex w-full items-end px-s-3 pb-s-1"
                  style={{
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  <h2 className="truncate text-xs font-semibold text-muted-foreground">
                    {entry.label}
                  </h2>
                </div>
              );
            }
            const item = items[entry.itemIndex];
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
                data-index={row.index}
                ref={isAndroidPlatform() ? virtualizer.measureElement : undefined}
                style={{
                  // Android image previews preserve their aspect ratio at the
                  // available row width. Let TanStack remeasure after the
                  // lazy thumbnail loads; desktop remains a fixed virtual row.
                  ...(isAndroidPlatform() ? { minHeight: row.size } : { height: row.size }),
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
                  origin={originLabel(item, marked)}
                  selecting={selection.selecting}
                  checked={selection.selected.has(item.id)}
                  onToggleChecked={(clicked) => selection.toggle(clicked.id)}
                  onSelect={(clicked) => onActiveIdChange(clicked.id)}
                  onCopy={copy}
                  onTogglePin={onTogglePin}
                  onDelete={onDelete}
                  onReveal={onReveal}
                  onOpen={onOpen}
                />
              </div>
            );
          })}
        </div>

        {/* An explicit control, not only the near-bottom trigger: three rows do
            not scroll, so a short filtered list with a thousand unpaged matches
            behind it would strand the user. */}
        {hasMore && (
          <div className="flex justify-center py-s-3">
            <Button
              variant="outline"
              size="sm"
              disabled={loadingMore}
              onClick={onLoadMore}
            >
              <ChevronDown aria-hidden="true" />
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
