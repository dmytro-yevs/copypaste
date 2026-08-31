import {
    useCallback,
    useEffect,
    useMemo,
    useRef,
    useState,
    type KeyboardEvent,
} from "react";

import type { CardSelectionIntent } from "@/features/history/components/ClipCard";
import { rowLabel } from "@/features/history/model/clipPresentation";
import { keyboardPinnedOrder } from "@/features/history/model/historyEntries";
import { markedOrigins, originLabel } from "@/lib/itemOrigin";
import { HISTORY_LAYOUT_METRICS } from "@/features/history/model/virtualizationMetrics";
import type { Selection } from "@/features/history/hooks/useSelection";
import { useTranslation } from "@/i18n";
import { useViewportMetrics } from "@/hooks/useViewportMetrics";
import type { Item } from "@/lib/ipc";

interface KeyboardNavigationOptions {
    items: readonly Item[];
    activeId: string | null;
    onActiveIdChange: (id: string | null) => void;
    selection: Selection;
    onOpen: (item: Item) => void;
    onDelete: (item: Item) => void;
    onReorderPinned: (ids: readonly string[]) => void;
    scrollToItemIndex: (index: number) => void;
    activeRendered: boolean;
}

export function useHistoryKeyboardNavigation({
    items,
    activeId,
    onActiveIdChange,
    selection,
    onOpen,
    onDelete,
    onReorderPinned,
    scrollToItemIndex,
    activeRendered,
}: KeyboardNavigationOptions) {
    const inspectorVisible =
        useViewportMetrics().width >=
        HISTORY_LAYOUT_METRICS.inspector.visibleAtPx;
    const { t } = useTranslation();
    const [announcement, setAnnouncement] = useState("");
    const itemsRef = useRef(items);
    itemsRef.current = items;
    const marked = useMemo(() => markedOrigins(items), [items]);
    const markedRef = useRef(marked);
    markedRef.current = marked;

    useEffect(() => {
        const item = itemsRef.current.find(
            (candidate) => candidate.id === activeId,
        );
        setAnnouncement(
            item
                ? rowLabel(
                      item,
                      originLabel(item, markedRef.current),
                      undefined,
                      t,
                  )
                : "",
        );
    }, [activeId, t]);

    const onSelection = useCallback(
        (item: Item, intent: CardSelectionIntent) => {
            onActiveIdChange(item.id);
            if (intent === "range") selection.rangeTo(item.id);
            else selection.toggle(item.id);
        },
        [onActiveIdChange, selection],
    );

    const activateItem = useCallback(
        (item: Item) => {
            onActiveIdChange(item.id);
            if (!inspectorVisible) onOpen(item);
        },
        [inspectorVisible, onActiveIdChange, onOpen],
    );

    const moveTo = useCallback(
        (index: number, extend: boolean) => {
            const item = items[index];
            if (!item) return;
            scrollToItemIndex(index);
            onActiveIdChange(item.id);
            if (extend) selection.rangeTo(item.id);
        },
        [items, onActiveIdChange, scrollToItemIndex, selection],
    );

    const onKeyDown = useCallback(
        (event: KeyboardEvent<HTMLDivElement>) => {
            if (
                items.length === 0 ||
                (event.target as Element).closest("[data-reorder-handle]")
            )
                return;
            const command = event.metaKey || event.ctrlKey;
            if (command && event.key.toLowerCase() === "a") {
                event.preventDefault();
                selection.selectAll();
                return;
            }
            const index = activeId
                ? items.findIndex((item) => item.id === activeId)
                : -1;
            const item = index >= 0 ? items[index] : undefined;
            if (
                event.altKey &&
                item?.pinned &&
                (event.key === "ArrowUp" || event.key === "ArrowDown")
            ) {
                const order = keyboardPinnedOrder(
                    items,
                    item.id,
                    event.key === "ArrowUp" ? -1 : 1,
                );
                if (order) {
                    event.preventDefault();
                    onReorderPinned(order);
                    setAnnouncement(
                        `${rowLabel(item, originLabel(item, markedRef.current), undefined, t)} moved ${event.key === "ArrowUp" ? "up" : "down"}`,
                    );
                }
                return;
            }
            switch (event.key) {
                case "ArrowDown":
                    event.preventDefault();
                    moveTo(
                        index < 0 ? 0 : Math.min(index + 1, items.length - 1),
                        event.shiftKey,
                    );
                    break;
                case "ArrowUp":
                    event.preventDefault();
                    moveTo(
                        index < 0 ? items.length - 1 : Math.max(index - 1, 0),
                        event.shiftKey,
                    );
                    break;
                case "ArrowRight":
                    if (item) {
                        event.preventDefault();
                        onOpen(item);
                    }
                    break;
                case "Home":
                    event.preventDefault();
                    moveTo(0, event.shiftKey);
                    break;
                case "End":
                    event.preventDefault();
                    moveTo(items.length - 1, event.shiftKey);
                    break;
                case " ":
                    if (item) {
                        event.preventDefault();
                        onSelection(item, event.shiftKey ? "range" : "toggle");
                    }
                    break;
                case "Enter":
                    if (item) {
                        event.preventDefault();
                        if (command) onOpen(item);
                        else if (selection.active) onSelection(item, "toggle");
                        else activateItem(item);
                    }
                    break;
                case "Backspace":
                case "Delete":
                    if (item && !selection.active) {
                        event.preventDefault();
                        const next =
                            items[index + 1] ?? items[index - 1] ?? null;
                        onActiveIdChange(next?.id ?? null);
                        onDelete(item);
                    }
                    break;
                case "Escape":
                    if (selection.active) selection.clear();
                    else onActiveIdChange(null);
                    break;
                default:
                    break;
            }
        },
        [
            activeId,
            activateItem,
            items,
            moveTo,
            onActiveIdChange,
            onDelete,
            onOpen,
            onReorderPinned,
            onSelection,
            selection,
            t,
        ],
    );

    return {
        announcement,
        marked,
        onSelection,
        activateItem,
        onKeyDown,
        activeDescendant:
            activeRendered && activeId ? `history-row-${activeId}` : undefined,
    };
}
