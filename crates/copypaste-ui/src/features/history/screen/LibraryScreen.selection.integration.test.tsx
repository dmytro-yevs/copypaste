import { StrictMode, type ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { CAPTURE_KEY } from "@/hooks/useCapture";
import { HISTORY_HEAD_KEY, STATUS_KEY, historyKey } from "@/hooks/historyRefresh";
import { ViewportMetricsProvider } from "@/hooks/useViewportMetrics";
import type { Item } from "@/lib/ipc";
import { captureSnapshot, item, page, status, testClient } from "@/test/harness";
import { LibraryScreen } from "./LibraryScreen";

const ipc = vi.hoisted(() => ({
    captureState: vi.fn(),
    getStatus: vi.fn(),
    listItems: vi.fn(),
    searchItems: vi.fn(),
    setPinned: vi.fn(),
}));
const toast = vi.hoisted(() => ({
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
}));

vi.mock("sonner", () => ({ toast }));
vi.mock("@/lib/ipc", async (load) => ({
    ...(await load<typeof import("@/lib/ipc")>()),
    captureState: ipc.captureState,
    getStatus: ipc.getStatus,
    listItems: ipc.listItems,
    searchItems: ipc.searchItems,
    setPinned: ipc.setPinned,
}));

function checkedIds(): string[] {
    return screen
        .getAllByRole("listitem")
        .filter((row) => row.getAttribute("aria-checked") === "true")
        .map((row) => row.id.replace(/^history-row-/, ""));
}

function deferred<T>() {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>((accept) => {
        resolve = accept;
    });
    return { promise, resolve };
}

function providers(client: ReturnType<typeof testClient>) {
    return ({ children }: { children: ReactNode }) => (
        <StrictMode>
            <QueryClientProvider client={client}>
                <TooltipProvider>
                    <ViewportMetricsProvider>{children}</ViewportMetricsProvider>
                </TooltipProvider>
            </QueryClientProvider>
        </StrictMode>
    );
}

describe("LibraryScreen selection integration", () => {
    let shown: Item[];

    beforeEach(() => {
        shown = [
            item({ id: "first", content: "first entry" }),
            item({ id: "second", content: "second entry" }),
        ];
        ipc.listItems.mockReset();
        ipc.searchItems.mockReset();
        ipc.setPinned.mockReset();
        ipc.captureState.mockReset();
        ipc.getStatus.mockReset();
        toast.error.mockReset();
        toast.success.mockReset();
        toast.warning.mockReset();
        ipc.listItems.mockImplementation(async () => page(shown));
        ipc.searchItems.mockImplementation(async () => page([]));
        ipc.captureState.mockResolvedValue(captureSnapshot());
        ipc.getStatus.mockImplementation(async () =>
            status({ item_count: shown.length }),
        );
        ipc.setPinned.mockImplementation(async (id: string, pinned: boolean) => {
            const index = shown.findIndex((entry) => entry.id === id);
            const updated = { ...shown[index]!, pinned };
            shown = shown.map((entry) => (entry.id === id ? updated : entry));
            return updated;
        });
    });

    function renderScreen() {
        const client = testClient();
        client.setQueryData(STATUS_KEY, status({ item_count: shown.length }));
        client.setQueryData(CAPTURE_KEY, captureSnapshot());
        client.setQueryData(HISTORY_HEAD_KEY, page(shown));
        client.setQueryData(historyKey(""), {
            pages: [page(shown)],
            pageParams: [null],
        });
        return {
            user: userEvent.setup(),
            client,
            ...render(<LibraryScreen />, { wrapper: providers(client) }),
        };
    }

    async function selectBoth(user: ReturnType<typeof userEvent.setup>) {
        await user.click(
            screen.getByRole("checkbox", { name: /select first entry/i }),
        );
        await user.click(
            screen.getByRole("checkbox", { name: /select second entry/i }),
        );
        expect(checkedIds()).toEqual(["first", "second"]);
        return screen.getByRole("toolbar", { name: "Selection actions" });
    }

    function publish(client: ReturnType<typeof testClient>) {
        client.setQueryData(HISTORY_HEAD_KEY, page(shown));
        client.setQueryData(historyKey(""), {
            pages: [page(shown)],
            pageParams: [null],
        });
    }

    function expectLibraryToolbarReplaced(selectionToolbar: HTMLElement) {
        const libraryToolbar = screen.getByRole("toolbar", {
            name: "Library controls",
        });

        expect(libraryToolbar).not.toBe(selectionToolbar);
        expect(selectionToolbar.isConnected).toBe(false);
        expect(libraryToolbar.getAttribute("aria-label")).toBe(
            "Library controls",
        );
        expect(screen.getAllByRole("toolbar")).toEqual([libraryToolbar]);
        expect(checkedIds()).toEqual([]);
    }

    it("ends selection through the rendered Done icon button", async () => {
        const { user } = renderScreen();
        const toolbar = await selectBoth(user);

        await user.click(within(toolbar).getByRole("button", { name: "Done" }));

        expect(
            screen.queryByRole("toolbar", { name: "Selection actions" }),
        ).toBeNull();
        expectLibraryToolbarReplaced(toolbar);
    });

    it("reconciles a successful bulk pin into the rendered rows", async () => {
        const first = deferred<Item>();
        const second = deferred<Item>();
        ipc.setPinned
            .mockImplementationOnce(() => first.promise)
            .mockImplementationOnce(() => second.promise);
        const { client, user } = renderScreen();
        const toolbar = await selectBoth(user);

        await user.click(within(toolbar).getByRole("button", { name: "Pin" }));

        expect(
            within(toolbar)
                .getByRole("button", { name: "Pin" })
                .hasAttribute("disabled"),
        ).toBe(true);
        const [firstItem, secondItem] = shown;
        shown = [{ ...firstItem!, pinned: true }, secondItem!];
        publish(client);
        first.resolve(shown[0]!);
        await waitFor(() => expect(ipc.setPinned).toHaveBeenCalledTimes(2));
        shown = shown.map((entry) => ({ ...entry, pinned: true }));
        publish(client);
        second.resolve({ ...secondItem!, pinned: true });
        await waitFor(() =>
            expect(
                screen.queryByRole("toolbar", { name: "Selection actions" }),
            ).toBeNull(),
        );
        expectLibraryToolbarReplaced(toolbar);
        expect(ipc.setPinned).toHaveBeenCalledTimes(2);
        expect(toast.success).toHaveBeenCalledWith("Pinned 2 items");
        await waitFor(() => {
            expect(
                screen.getAllByRole("listitem").filter(
                    (row) =>
                        row.id.startsWith("history-row-") &&
                        row.textContent?.includes("Pinned"),
                ),
            ).toHaveLength(2);
        });
    });

    it("keeps a failed row selected after a partial pin", async () => {
        const first = shown[0]!;
        const second = shown[1]!;
        ipc.setPinned
            .mockImplementationOnce(async () => {
                shown = [{ ...first, pinned: true }, second];
                return shown[0]!;
            })
            .mockRejectedValueOnce(new Error("service unavailable"));
        const { user } = renderScreen();
        const toolbar = await selectBoth(user);

        await user.click(within(toolbar).getByRole("button", { name: "Pin" }));

        await waitFor(() => expect(checkedIds()).toEqual(["second"]));
        expect(
            screen.getByRole("toolbar", { name: "Selection actions" }),
        ).not.toBeNull();
        expect(toast.warning).toHaveBeenCalledWith(
            "Pinned 1 of 2 — 1 failed",
        );
    });

    it("does not drop a selection made while a pin is in flight", async () => {
        const third = item({ id: "third", content: "third entry" });
        shown = [...shown, third];
        const first = deferred<Item>();
        const second = deferred<Item>();
        ipc.setPinned
            .mockImplementationOnce(() => first.promise)
            .mockImplementationOnce(() => second.promise);
        const { client, user } = renderScreen();
        const toolbar = await selectBoth(user);

        await user.click(within(toolbar).getByRole("button", { name: "Pin" }));
        await user.click(
            screen.getByRole("checkbox", { name: /select third entry/i }),
        );
        expect(checkedIds()).toEqual(["first", "second", "third"]);

        shown = shown.map((entry, index) =>
            index === 0 ? { ...entry, pinned: true } : entry,
        );
        publish(client);
        first.resolve(shown[0]!);
        await waitFor(() => expect(ipc.setPinned).toHaveBeenCalledTimes(2));
        shown = shown.map((entry, index) =>
            index < 2 ? { ...entry, pinned: true } : entry,
        );
        publish(client);
        second.resolve(shown[1]!);

        await waitFor(() => expect(checkedIds()).toEqual(["third"]));
        expect(
            screen.getByRole("toolbar", { name: "Selection actions" }),
        ).not.toBeNull();
    });
});
