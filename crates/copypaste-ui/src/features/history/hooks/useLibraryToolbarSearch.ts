import {
    useCallback,
    useEffect,
    useId,
    useRef,
    useState,
    type KeyboardEvent as ReactKeyboardEvent,
    type RefObject,
} from "react";

import { HISTORY_LAYOUT_METRICS } from "@/features/history/model/virtualizationMetrics";
import { useObservedElementSize } from "@/hooks/useViewportMetrics";

interface LibraryToolbarSearchOptions {
    inputRef: RefObject<HTMLInputElement | null>;
    selectionActive: boolean;
    value: string;
    onChange: (value: string) => void;
    onEnterList: () => void;
}

export function useLibraryToolbarSearch({
    inputRef,
    selectionActive,
    value,
    onChange,
    onEnterList,
}: LibraryToolbarSearchOptions) {
    const toolbarRef = useRef<HTMLDivElement>(null);
    const { ref: observedToolbarRef, width: toolbarWidth } =
        useObservedElementSize<HTMLDivElement>();
    const siblingsRef = useRef<HTMLDivElement>(null);
    const overlayRef = useRef<HTMLInputElement>(null);
    const searchTriggerId = useId();
    const searchOverlayId = useId();
    const [searchExpanded, setSearchExpanded] = useState(false);
    const compactSearch =
        toolbarWidth > 0 &&
        toolbarWidth <= HISTORY_LAYOUT_METRICS.toolbar.compactSearchMaxPx;

    const collapseSearch = useCallback(() => setSearchExpanded(false), []);
    const closeSearch = useCallback(() => {
        setSearchExpanded(false);
        requestAnimationFrame(() =>
            document.getElementById(searchTriggerId)?.focus(),
        );
    }, [searchTriggerId]);
    const setToolbarRef = useCallback(
        (element: HTMLDivElement | null) => {
            toolbarRef.current = element;
            observedToolbarRef(element);
        },
        [observedToolbarRef],
    );
    const openSearch = useCallback(
        (selectContents = false) => {
            if (!compactSearch) {
                inputRef.current?.focus();
                if (selectContents) inputRef.current?.select();
                return;
            }
            setSearchExpanded(true);
            requestAnimationFrame(() => {
                overlayRef.current?.focus();
                if (selectContents) overlayRef.current?.select();
            });
        },
        [compactSearch, inputRef],
    );

    useEffect(() => {
        if (!compactSearch || selectionActive) setSearchExpanded(false);
    }, [compactSearch, selectionActive]);

    useEffect(() => {
        if (siblingsRef.current) siblingsRef.current.inert = searchExpanded;
    }, [searchExpanded]);

    useEffect(() => {
        const onShortcut = (event: KeyboardEvent) => {
            if (
                !(event.metaKey || event.ctrlKey) ||
                !["f", "k"].includes(event.key.toLowerCase())
            )
                return;
            event.preventDefault();
            openSearch(true);
        };
        const onOutside = (event: PointerEvent) => {
            if (
                searchExpanded &&
                toolbarRef.current &&
                !toolbarRef.current.contains(event.target as Node)
            ) {
                collapseSearch();
            }
        };
        window.addEventListener("keydown", onShortcut);
        document.addEventListener("pointerdown", onOutside);
        return () => {
            window.removeEventListener("keydown", onShortcut);
            document.removeEventListener("pointerdown", onOutside);
        };
    }, [collapseSearch, openSearch, searchExpanded]);

    const handleSearchKey = (event: ReactKeyboardEvent<HTMLInputElement>) => {
        if (event.key === "ArrowDown") {
            event.preventDefault();
            collapseSearch();
            onEnterList();
        } else if (event.key === "Escape") {
            event.preventDefault();
            if (value) onChange("");
            else closeSearch();
        } else if (event.key === "Tab" && searchExpanded) {
            event.preventDefault();
            collapseSearch();
            requestAnimationFrame(() => {
                if (event.shiftKey)
                    document.getElementById(searchTriggerId)?.focus();
                else
                    toolbarRef.current
                        ?.querySelector<HTMLElement>(
                            '[data-slot="select-trigger"]',
                        )
                        ?.focus();
            });
        }
    };

    return {
        closeSearch,
        compactSearch,
        handleSearchKey,
        openSearch,
        overlayRef,
        searchExpanded,
        searchOverlayId,
        searchTriggerId,
        setToolbarRef,
        siblingsRef,
    };
}
