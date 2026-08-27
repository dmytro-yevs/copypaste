/**
 * INV-1 (content-anchored scrolling) and INV-6 (the shrink clamp). TanStack
 * Virtual does not preserve either for a mutating list, so the arithmetic is
 * owned here. The anchor is item id plus intra-row offset, never an index.
 * At `scrollTop === 0` nothing is anchored, so prepends remain visible.
 */
import { type RefObject, useCallback, useLayoutEffect, useRef } from "react";
import type { Virtualizer } from "@tanstack/react-virtual";

import type { Item } from "@/lib/ipc";

interface Anchor {
    readonly id: string;
    /** Distance from the top of the anchored row to the viewport top. */
    readonly offset: number;
}

interface AppliedAnchor {
    readonly id: string;
    readonly start: number;
}

interface PendingScroll {
    readonly id: string | null;
    readonly offset: number;
}

interface Options {
    scrollRef: RefObject<HTMLDivElement | null>;
    virtualizer: Virtualizer<HTMLDivElement, Element>;
    items: readonly Item[];
    /** `null` marks a non-item virtual row such as a device heading. */
    anchorIds?: readonly (string | null)[];
    /** Triggers INV-6 immediately when a preview-line change shrinks rows. */
    previewLines: number;
}

export function useScrollAnchor({
    scrollRef,
    virtualizer,
    items,
    anchorIds,
    previewLines,
}: Options) {
    const anchorRef = useRef<Anchor | null>(null);
    const appliedAnchorRef = useRef<AppliedAnchor | null>(null);
    const pendingScrollRef = useRef<PendingScroll | null>(null);
    const previousItems = useRef<readonly Item[] | null>(null);
    const previousAnchorIds = useRef<readonly (string | null)[] | undefined>(
        anchorIds,
    );
    const previousLines = useRef(previewLines);
    const previousMax = useRef<number | null>(null);

    /** Resolves rows outside the rendered window from the measurement table. */
    const captureAnchor = useCallback(() => {
        const element = scrollRef.current;
        if (!element) return;

        const top = element.scrollTop;
        if (top <= 0) {
            anchorRef.current = null;
            appliedAnchorRef.current = null;
            pendingScrollRef.current = null;
            return;
        }

        const pending = pendingScrollRef.current;
        if (
            pending &&
            Math.abs(top - pending.offset) < 1.5 &&
            (pending.id === null || anchorRef.current?.id === pending.id)
        ) {
            pendingScrollRef.current = null;
            return;
        }
        pendingScrollRef.current = null;

        const row = virtualizer.getVirtualItemForOffset(top);
        if (!row) {
            anchorRef.current = null;
            appliedAnchorRef.current = null;
            return;
        }

        const ids = anchorIds ?? items.map((item) => item.id);
        let index = row.index;
        while (index < ids.length && ids[index] === null) index += 1;
        const id = ids[index];
        const measurement = virtualizer.measurementsCache[index];
        anchorRef.current =
            id && measurement ? { id, offset: top - measurement.start } : null;
        // A captured offset establishes a new point of interest. The next
        // layout pass may be a no-op, but must not reuse prior geometry.
        appliedAnchorRef.current = null;
    }, [anchorIds, items, scrollRef, virtualizer]);

    useLayoutEffect(() => {
        const element = scrollRef.current;
        if (!element) return;

        const linesChanged = previousLines.current !== previewLines;
        previousLines.current = previewLines;
        const itemsChanged = previousItems.current !== items;
        const anchorIdsChanged = previousAnchorIds.current !== anchorIds;
        previousAnchorIds.current = anchorIds;
        const hadItems = previousItems.current !== null;
        previousItems.current = items;
        const max = Math.max(
            0,
            virtualizer.getTotalSize() - element.clientHeight,
        );
        const maxChanged =
            previousMax.current !== null && previousMax.current !== max;
        previousMax.current = max;
        if (!hadItems) return;

        const anchor = anchorRef.current;
        const ids = anchorIds ?? items.map((item) => item.id);
        const index = anchor ? ids.indexOf(anchor.id) : -1;
        const measurement =
            index >= 0 ? virtualizer.measurementsCache[index] : undefined;
        const geometryChanged =
            anchor !== null &&
            measurement !== undefined &&
            (appliedAnchorRef.current === null ||
                appliedAnchorRef.current.id !== anchor.id ||
                appliedAnchorRef.current.start !== measurement.start);
        const mutation = itemsChanged || anchorIdsChanged || linesChanged;
        if (!mutation && !maxChanged && !geometryChanged) return;

        let desired = element.scrollTop;

        if (anchor && measurement) {
            desired = measurement.start + anchor.offset;
            appliedAnchorRef.current = {
                id: anchor.id,
                start: measurement.start,
            };
        } else {
            appliedAnchorRef.current = null;
            // A deleted anchor falls through to the shrink clamp.
        }

        const next = Math.min(Math.max(desired, 0), max);
        if (Math.abs(next - element.scrollTop) > 0.5) {
            // Keeps the DOM position and virtualizer offset in the same frame.
            pendingScrollRef.current = {
                id: anchor?.id ?? null,
                offset: next,
            };
            virtualizer.scrollToOffset(next, {
                align: "start",
                behavior: "auto",
            });
        } else {
            pendingScrollRef.current = null;
        }
    });

    return { captureAnchor };
}
