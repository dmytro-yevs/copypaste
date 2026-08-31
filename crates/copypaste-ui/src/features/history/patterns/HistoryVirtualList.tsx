import type { VirtualItem } from "@tanstack/react-virtual";

import {
  ClipCard,
  type CardSelectionIntent,
} from "@/features/history/components/ClipCard";
import type { HistoryEntry } from "@/features/history/model/historyEntries";
import { markedOrigin } from "@/features/history/model/origin";
import { ClipImageLoader } from "@/features/history/patterns/ClipImageLoader";
import { kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import {
  SortablePinned,
  type SortableBindings,
} from "./PinnedReorder";
import styles from "./HistoryVirtualList.module.css";

interface HistoryVirtualListProps {
  entries: readonly HistoryEntry[];
  rows: readonly VirtualItem[];
  totalSize: number;
  items: readonly Item[];
  activeId: string | null;
  revealedId: string | null;
  revealedContent: string | null;
  revealPendingId: string | null;
  previewLines: number;
  selectionActive: boolean;
  selected: ReadonlySet<string>;
  markedOrigins: ReadonlySet<string>;
  pinnedPositions: ReadonlyMap<string, number>;
  onSelection: (item: Item, intent: CardSelectionIntent) => void;
  onActivate: (item: Item) => void;
  measureElement: (element: Element | null) => void;
}

export function HistoryVirtualList({
  entries,
  rows,
  totalSize,
  items,
  activeId,
  revealedId,
  revealedContent,
  revealPendingId,
  previewLines,
  selectionActive,
  selected,
  markedOrigins,
  pinnedPositions,
  onSelection,
  onActivate,
  measureElement,
}: HistoryVirtualListProps) {
  return (
    <div className={styles.canvas} style={{ height: totalSize }}>
      {rows.map((row) => {
        const entry = entries[row.index];
        if (!entry) return null;

        if (entry.type === "group") {
          return (
            <div
              key={row.key}
              role="listitem"
              className={styles.group}
              data-index={row.index}
              ref={measureElement}
              style={{ transform: `translateY(${row.start}px)` }}
            >
              <h2 className={styles.groupTitle} title={entry.label}>
                {entry.label}
              </h2>
            </div>
          );
        }

        const item = items[entry.itemIndex];
        if (!item) return null;
        const renderItem = (sortable?: SortableBindings) => (
          <div
            key={row.key}
            id={`history-row-${item.id}`}
            role="listitem"
            // The card owns the accessible name; naming this wrapper would
            // announce every virtual row twice.
            aria-current={item.id === activeId ? "true" : undefined}
            className={styles.slot}
            data-index={row.index}
            data-dragging={sortable?.dragging || undefined}
            data-drop-target={sortable?.dropTarget || undefined}
            ref={(element) => {
              sortable?.elementRef(element);
              measureElement(element);
            }}
            style={{ transform: `translateY(${row.start}px)` }}
          >
            <ClipCard
              item={item}
              state={
                sortable?.dragging
                  ? "dragging"
                  : item.id === revealPendingId
                    ? "pending"
                    : selected.has(item.id)
                      ? "checked"
                      : item.id === activeId
                        ? "active"
                        : "default"
              }
              revealedContent={
                item.id === revealedId ? revealedContent : null
              }
              revealPending={item.id === revealPendingId}
              origin={markedOrigin(item, markedOrigins)}
              selectionActive={selectionActive}
              checked={selected.has(item.id)}
              previewLines={previewLines}
              imagePreview={
                kindOf(item) === "image"
                  ? <ClipImageLoader id={item.id} size="fill" />
                  : undefined
              }
              onSelection={onSelection}
              onActivate={onActivate}
            />
          </div>
        );
        const pinnedIndex = pinnedPositions.get(item.id);

        return pinnedIndex === undefined ? (
          renderItem()
        ) : (
          <SortablePinned key={row.key} id={item.id} index={pinnedIndex}>
            {renderItem}
          </SortablePinned>
        );
      })}
    </div>
  );
}
