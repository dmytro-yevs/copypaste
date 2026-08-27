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
) {
    let currentItems = initialItems;
    let size = initialSize;
    let measurements = measure(currentItems, size);
    const scrollToOffset = vi.fn((offset: number) => {
        element.scrollTop = offset;
    });
    const virtualizer = {
        getTotalSize: () => currentItems.length * size,
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
        setGeometry(nextItems: readonly Item[], nextSize: number) {
            currentItems = nextItems;
            size = nextSize;
            measurements = measure(currentItems, size);
        },
    };
}

function measure(list: readonly Item[], size: number) {
    return list.map((entry, index) => ({
        index,
        key: entry.id,
        start: index * size,
        end: (index + 1) * size,
        size,
        lane: 0,
    }));
}

function setup(list: readonly Item[], scrollTop: number) {
    const element = document.createElement("div");
    Object.defineProperty(element, "clientHeight", { value: VIEWPORT });
    element.scrollTop = scrollTop;
    document.body.appendChild(element);

    const scrollRef = createRef<HTMLDivElement>();
    (scrollRef as { current: HTMLDivElement }).current = element;
    const fake = makeVirtualizer(element, list, ROW);
    const rendered = renderHook(
        ({ current, size }: { current: readonly Item[]; size: number }) => {
            fake.setGeometry(current, size);
            return useScrollAnchor({
                scrollRef,
                virtualizer: fake.virtualizer,
                items: current,
                previewLines: 2,
            });
        },
        { initialProps: { current: list, size: ROW } },
    );

    return { element, fake, ...rendered };
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
});
