import { createRef } from "react";
import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Virtualizer } from "@tanstack/react-virtual";

import { items } from "@/test/harness";
import type { Item } from "@/lib/ipc";
import { rowHeight } from "@/features/history/model/virtualizationMetrics";
import { useScrollAnchor } from "./useScrollAnchor";

const ROW = rowHeight(2);
const VIEWPORT = 400;

function makeVirtualizer(
    element: HTMLDivElement,
    initialItems: readonly Item[],
    initialSize: number,
    initialAnchorIds?: readonly (string | null)[],
    initialTailClearance = 0,
) {
    let currentItems = initialItems;
    let size = initialSize;
    let anchorIds = initialAnchorIds;
    let tailClearance = initialTailClearance;
    let measurements = measure(anchorIds ?? currentItems, size);
    const scrollToOffset = vi.fn((offset: number) => {
        element.scrollTop = offset;
    });
    const virtualizer = {
        getTotalSize: () => measurements.length * size,
        getVirtualItemForOffset: (offset: number) =>
            measurements[
                Math.min(measurements.length - 1, Math.floor(offset / size))
            ],
        get measurementsCache() {
            return measurements;
        },
        scrollToOffset,
    } as unknown as Virtualizer<HTMLDivElement, Element>;

    return {
        virtualizer,
        scrollToOffset,
        getScrollHeight() {
            return measurements.length * size + tailClearance;
        },
        setGeometry(
            nextItems: readonly Item[],
            nextSize: number,
            nextAnchorIds?: readonly (string | null)[],
            nextTailClearance = tailClearance,
        ) {
            currentItems = nextItems;
            size = nextSize;
            anchorIds = nextAnchorIds;
            tailClearance = nextTailClearance;
            measurements = measure(anchorIds ?? currentItems, size);
        },
    };
}

function measure(list: readonly (Item | string | null)[], size: number) {
    return list.map((entry, index) => ({
        index,
        key:
            typeof entry === "object" && entry !== null
                ? entry.id
                : (entry ?? `group:${index}`),
        start: index * size,
        end: (index + 1) * size,
        size,
        lane: 0,
    }));
}

function setup(
    list: readonly Item[],
    scrollTop: number,
    {
        anchorIds,
        tailClearance = 0,
        viewportHeight = VIEWPORT,
    }: {
        anchorIds?: readonly (string | null)[];
        tailClearance?: number;
        viewportHeight?: number;
    } = {},
) {
    const element = document.createElement("div");
    Object.defineProperty(element, "clientHeight", { value: viewportHeight });
    element.scrollTop = scrollTop;
    document.body.appendChild(element);

    const scrollRef = createRef<HTMLDivElement>();
    (scrollRef as { current: HTMLDivElement }).current = element;
    const fake = makeVirtualizer(
        element,
        list,
        ROW,
        anchorIds,
        tailClearance,
    );
    Object.defineProperty(element, "scrollHeight", {
        configurable: true,
        get: () => fake.getScrollHeight(),
    });
    const rendered = renderHook(
        ({
            current,
            size,
            currentAnchorIds,
            currentTailClearance,
        }: {
            current: readonly Item[];
            size: number;
            currentAnchorIds?: readonly (string | null)[];
            currentTailClearance?: number;
        }) => {
            fake.setGeometry(
                current,
                size,
                currentAnchorIds,
                currentTailClearance,
            );
            return useScrollAnchor({
                scrollRef,
                virtualizer: fake.virtualizer,
                items: current,
                anchorIds: currentAnchorIds,
                previewLines: 2,
            });
        },
        {
            initialProps: {
                current: list,
                size: ROW,
                currentAnchorIds: anchorIds,
                currentTailClearance: undefined as number | undefined,
            },
        },
    );

    return {
        element,
        fake,
        result: rendered.result,
        unmount: rendered.unmount,
        rerender({
            current,
            size,
            currentAnchorIds,
            currentTailClearance,
        }: {
            current: readonly Item[];
            size: number;
            currentAnchorIds?: readonly (string | null)[];
            currentTailClearance?: number;
        }) {
            rendered.rerender({
                current,
                size,
                currentAnchorIds,
                currentTailClearance,
            });
        },
    };
}

describe("useScrollAnchor", () => {
    it("keeps the item and intra-row offset through a prepend", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, ROW * 5 + 17);
        result.current.captureAnchor();

        const prepended = [{ ...items(1)[0]!, id: "brand-new" }, ...list];
        rerender({ current: prepended, size: ROW });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            ROW * 6 + 17,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("reconciles the anchor after a later measurement changes row geometry", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, ROW * 5);
        result.current.captureAnchor();

        const prepended = [{ ...items(1)[0]!, id: "brand-new" }, ...list];
        rerender({ current: prepended, size: ROW });
        rerender({ current: prepended, size: ROW + 10 });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            (ROW + 10) * 6,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("does not recapture the wrong row from a correction scroll event", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, ROW * 5);
        result.current.captureAnchor();

        const prepended = [{ ...items(1)[0]!, id: "brand-new" }, ...list];
        rerender({ current: prepended, size: ROW });
        fake.setGeometry(prepended, ROW + 10);
        result.current.captureAnchor();
        rerender({ current: prepended, size: ROW + 10 });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            (ROW + 10) * 6,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("reveals a prepended item when the viewport is at scrollTop zero", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, 0);
        result.current.captureAnchor();

        rerender({
            current: [{ ...items(1)[0]!, id: "brand-new" }, ...list],
            size: ROW,
        });

        expect(fake.scrollToOffset).not.toHaveBeenCalled();
    });

    it("clamps when the anchor row is removed and the list shrinks", () => {
        const list = items(20);
        const { element, result, rerender, fake } = setup(list, ROW * 15);
        result.current.captureAnchor();

        rerender({ current: list.slice(0, 4), size: ROW });

        expect(fake.scrollToOffset).toHaveBeenCalled();
        expect(element.scrollTop).toBe(4 * ROW - VIEWPORT);
    });

    it("uses the next surviving item when the anchored item is deleted", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, ROW * 5 + 17);
        result.current.captureAnchor();

        const withoutAnchor = list.filter((_, index) => index !== 5);
        rerender({ current: withoutAnchor, size: ROW });
        rerender({ current: withoutAnchor, size: ROW + 10 });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            (ROW + 10) * 5 + 17,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("uses the previous surviving item when no later item survives", () => {
        const list = items(20);
        const { result, rerender, fake } = setup(list, ROW * 15 + 17, {
            viewportHeight: 80,
        });
        result.current.captureAnchor();

        rerender({ current: list.slice(0, 15), size: ROW });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            ROW * 14 + 17,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("uses prior grouped-row distance to select the nearest survivor", () => {
        const list = items(3);
        const ids = [null, list[0]!.id, list[1]!.id, null, list[2]!.id];
        const { result, rerender, fake } = setup(list, ROW * 2 + 17, {
            anchorIds: ids,
            viewportHeight: 80,
        });
        result.current.captureAnchor();

        rerender({
            current: [list[0]!, list[2]!],
            size: ROW,
            currentAnchorIds: [null, list[0]!.id, null, list[2]!.id],
        });

        expect(fake.scrollToOffset).toHaveBeenLastCalledWith(
            ROW + 17,
            expect.objectContaining({ align: "start", behavior: "auto" }),
        );
    });

    it("clamps when the load-more tail clearance is removed", () => {
        const list = items(5);
        const { element, result, rerender, fake } = setup(list, 250, {
            tailClearance: 120,
        });
        result.current.captureAnchor();

        rerender({ current: list, size: ROW, currentTailClearance: 0 });

        expect(fake.scrollToOffset).toHaveBeenCalled();
        expect(element.scrollTop).toBe(5 * ROW - VIEWPORT);
    });
});
