import {
    useCallback,
    useLayoutEffect,
    useMemo,
    type RefObject,
    type UIEvent,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import {
    historyEntries,
    historyEntryKey,
} from "@/features/history/model/historyEntries";
import {
    HISTORY_LAYOUT_METRICS,
    historyRowEstimate,
    overscanRows,
} from "@/features/history/model/virtualizationMetrics";
import { useScrollAnchor } from "@/features/history/hooks/useScrollAnchor";
import { useSizeClass } from "@/hooks/useSizeClass";
import { kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export function useHistoryVirtualizer({
    items,
    groupedByDevice,
    unknownDeviceLabel,
    previewLines,
    listRef,
    onLoadMore,
}: {
    items: readonly Item[];
    groupedByDevice: boolean;
    unknownDeviceLabel: string;
    previewLines: number;
    listRef: RefObject<HTMLDivElement | null>;
    onLoadMore: () => void;
}) {
    const compact = useSizeClass() === "compact";
    const entries = useMemo(
        () => historyEntries(items, groupedByDevice, unknownDeviceLabel),
        [groupedByDevice, items, unknownDeviceLabel],
    );
    const anchorIds = useMemo(
        () =>
            entries.map((entry) =>
                entry.type === "item"
                    ? (items[entry.itemIndex]?.id ?? null)
                    : null,
            ),
        [entries, items],
    );
    const estimateSize = useCallback(
        (index: number) => {
            const entry = entries[index];
            if (entry?.type === "group")
                return historyRowEstimate("group", previewLines, compact);
            const item = items[entry?.itemIndex ?? -1];
            if (!item)
                return historyRowEstimate("unknown", previewLines, compact);
            return historyRowEstimate(kindOf(item), previewLines, compact);
        },
        [compact, entries, items, previewLines],
    );
    const virtualizer = useVirtualizer({
        count: entries.length,
        getScrollElement: () => listRef.current,
        estimateSize,
        getItemKey: (index) => {
            const entry = entries[index];
            return entry ? historyEntryKey(entry, items) : `missing:${index}`;
        },
        overscan: overscanRows(previewLines),
    });
    const entryOrder = useMemo(
        () =>
            entries
                .map((entry) => historyEntryKey(entry, items))
                .join("\u001f"),
        [entries, items],
    );
    useLayoutEffect(() => {
        virtualizer.measure();
    }, [compact, entryOrder, previewLines, virtualizer]);
    const { captureAnchor } = useScrollAnchor({
        scrollRef: listRef,
        virtualizer,
        items,
        anchorIds,
        previewLines,
    });
    const onScroll = useCallback(
        (event: UIEvent<HTMLDivElement>) => {
            captureAnchor();
            const element = event.currentTarget;
            if (
                element.scrollHeight -
                    element.scrollTop -
                    element.clientHeight <
                HISTORY_LAYOUT_METRICS.virtualizer.loadMoreThresholdPx
            ) {
                onLoadMore();
            }
        },
        [captureAnchor, onLoadMore],
    );
    const scrollToItemIndex = useCallback(
        (itemIndex: number) => {
            const virtualIndex = entries.findIndex(
                (entry) =>
                    entry.type === "item" && entry.itemIndex === itemIndex,
            );
            if (virtualIndex >= 0)
                virtualizer.scrollToIndex(virtualIndex, { align: "auto" });
        },
        [entries, virtualizer],
    );
    return {
        entries,
        virtualizer,
        virtualRows: virtualizer.getVirtualItems(),
        onScroll,
        scrollToItemIndex,
    };
}
