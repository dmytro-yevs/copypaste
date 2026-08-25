/**
 * INV-8: `role="list"`/`listitem`, never listbox/option — the rows carry
 * buttons, and option is `childrenPresentational` (axe `nested-interactive`).
 * INV-9/A11Y-14: the announcer is a *sibling* of the list; inside it, it fails
 * `aria-required-children` (CopyPaste-wrfn).
 */
import { type RefObject, useMemo } from "react";

import { ScrollViewport } from "@/components/layout";
import { Button, Icon, VisuallyHidden } from "@/components/ui";
import { useTranslation } from "@/i18n";
import type { Item } from "@/lib/ipc";
import { useHistoryKeyboardNavigation } from "@/features/history/hooks/useHistoryKeyboardNavigation";
import { useHistoryVirtualizer } from "@/features/history/hooks/useHistoryVirtualizer";
import { PinnedReorderProvider } from "@/features/history/patterns/PinnedReorder";
import { HistoryVirtualList } from "@/features/history/patterns/HistoryVirtualList";
import { historyEntryKey } from "@/features/history/model/historyEntries";
import type { Selection } from "@/features/history/hooks/useSelection";
import styles from "./HistoryList.module.css";

interface HistoryListProps {
    items: readonly Item[];
    activeId: string | null;
    onActiveIdChange: (id: string | null) => void;
    revealedId: string | null;
    revealedContent: string | null;
    revealPendingId: string | null;
    previewLines: number;
    groupedByDevice: boolean;
    selection: Selection;
    hasMore: boolean;
    loadingMore: boolean;
    onReorderPinned: (ids: readonly string[]) => void;
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
    groupedByDevice,
    selection,
    hasMore,
    loadingMore,
    onReorderPinned,
    onDelete,
    onOpen,
    onLoadMore,
    listRef,
}: HistoryListProps) {
    const { t } = useTranslation();
    const pinnedIds = useMemo(
        () => items.filter((item) => item.pinned).map((item) => item.id),
        [items],
    );
    const pinnedPositions = useMemo(
        () => new Map(pinnedIds.map((id, index) => [id, index])),
        [pinnedIds],
    );

    const { entries, virtualizer, virtualRows, onScroll, scrollToItemIndex } =
        useHistoryVirtualizer({
            items,
            groupedByDevice,
            unknownDeviceLabel: t("history.list.unknownDevice"),
            previewLines,
            listRef,
            onLoadMore,
        });

    const activeRendered =
        activeId !== null &&
        virtualRows.some((row) => {
            const entry = entries[row.index];
            return (
                entry?.type === "item" &&
                historyEntryKey(entry, items) === `item:${activeId}`
            );
        });
    const {
        announcement,
        marked,
        onSelection,
        activateItem,
        onKeyDown,
        activeDescendant,
    } = useHistoryKeyboardNavigation({
        items,
        activeId,
        onActiveIdChange,
        selection,
        onOpen,
        onDelete,
        onReorderPinned,
        scrollToItemIndex,
        activeRendered,
    });

    return (
        <div className={styles.root}>
            <PinnedReorderProvider ids={pinnedIds} onReorder={onReorderPinned}>
                <ScrollViewport
                    ref={listRef}
                    role="list"
                    aria-label={t("history.list.label")}
                    focusable
                    onKeyDown={onKeyDown}
                    onScroll={onScroll}
                    data-active-descendant={activeDescendant}
                    className={styles.viewport}
                >
                    <HistoryVirtualList
                        entries={entries}
                        rows={virtualRows}
                        totalSize={virtualizer.getTotalSize()}
                        items={items}
                        activeId={activeId}
                        revealedId={revealedId}
                        revealedContent={revealedContent}
                        revealPendingId={revealPendingId}
                        previewLines={previewLines}
                        selectionActive={selection.active}
                        selected={selection.selected}
                        markedOrigins={marked}
                        pinnedPositions={pinnedPositions}
                        onSelection={onSelection}
                        onActivate={activateItem}
                        measureElement={virtualizer.measureElement}
                    />

                    {/* An explicit control, not only the near-bottom trigger: three rows do
            not scroll, so a short filtered list with a thousand unpaged matches
            behind it would strand the user. */}
                    {hasMore && (
                        <div className={styles.loadMore}>
                            <Button
                                variant="secondary"
                                size="sm"
                                disabled={loadingMore}
                                onClick={onLoadMore}
                            >
                                <Icon name="caretDown" size="sm" />
                                {t(
                                    loadingMore
                                        ? "history.list.loadingMore"
                                        : "history.list.loadMore",
                                )}
                            </Button>
                        </div>
                    )}
                </ScrollViewport>
            </PinnedReorderProvider>

            {/* INV-9 / A11Y-14: a sibling of the list, never a child of it. */}
            <VisuallyHidden aria-live="polite" aria-atomic="true">
                {announcement}
            </VisuallyHidden>
        </div>
    );
}
