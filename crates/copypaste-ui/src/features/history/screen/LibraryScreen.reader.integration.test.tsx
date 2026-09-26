import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { CAPTURE_KEY } from "@/hooks/useCapture";
import { HISTORY_HEAD_KEY, STATUS_KEY, historyKey } from "@/hooks/historyRefresh";
import { ViewportMetricsProvider } from "@/hooks/useViewportMetrics";
import { useUi } from "@/store/ui";
import { captureSnapshot, item, page, status, testClient } from "@/test/harness";
import { LibraryScreen } from "./LibraryScreen";

const ipc = vi.hoisted(() => ({
    captureState: vi.fn(),
    getStatus: vi.fn(),
    listItems: vi.fn(),
    searchItems: vi.fn(),
    getItemBody: vi.fn(),
    copyItem: vi.fn(),
}));
const viewport = vi.hoisted(() => ({ width: 1200 }));
const toast = vi.hoisted(() => ({
    error: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
}));

vi.mock("sonner", () => ({ toast }));

vi.mock("@/hooks/useViewportMetrics", async (load) => ({
    ...(await load<typeof import("@/hooks/useViewportMetrics")>()),
    useViewportMetrics: () => ({
        width: viewport.width,
        height: 844,
        pointer: "fine" as const,
        sizeClass: viewport.width >= 640 ? "expanded" as const : "compact" as const,
    }),
}));

vi.mock("@/lib/ipc", async (load) => ({
    ...(await load<typeof import("@/lib/ipc")>()),
    ...ipc,
}));

const longBody = "full line\n".repeat(200);
const longItem = item({ id: "long", content: "short preview", truncated: true });

function renderScreen() {
    const client = testClient();
    const items = [longItem];
    client.setQueryData(STATUS_KEY, status({ item_count: 1 }));
    client.setQueryData(CAPTURE_KEY, captureSnapshot());
    client.setQueryData(HISTORY_HEAD_KEY, page(items));
    client.setQueryData(historyKey(""), {
        pages: [page(items)],
        pageParams: [null],
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>
            <TooltipProvider>
                <ViewportMetricsProvider>{children}</ViewportMetricsProvider>
            </TooltipProvider>
        </QueryClientProvider>
    );
    return { user: userEvent.setup(), ...render(<LibraryScreen />, { wrapper }) };
}

describe("LibraryScreen reader reachability", () => {
    beforeEach(() => {
        window.sessionStorage.clear();
        useUi.setState({ activeId: null, query: "" });
        ipc.captureState.mockReset().mockResolvedValue(captureSnapshot());
        ipc.getStatus.mockReset().mockResolvedValue(status({ item_count: 1 }));
        ipc.listItems.mockReset().mockResolvedValue(page([longItem]));
        ipc.searchItems.mockReset().mockResolvedValue(page([]));
        ipc.getItemBody.mockReset().mockResolvedValue(longBody);
        ipc.copyItem.mockReset().mockResolvedValue(undefined);
        toast.error.mockReset();
        toast.success.mockReset();
        toast.warning.mockReset();
    });

    it("opens the full, scrollable reader from desktop selection and returns focus", async () => {
        viewport.width = 1200;
        const { user, container } = renderScreen();
        const trigger = await screen.findByRole("button", { name: "short preview" });

        await user.click(trigger);
        const inspector = await screen.findByRole("complementary", { name: "Inspector" });
        const openReader = within(inspector).getByRole("button", { name: "Show full contents" });
        await user.click(openReader);
        const dialog = await screen.findByRole("dialog", { name: "Clipboard item" });
        await waitFor(() =>
            expect(within(dialog).getByRole("region", { name: "Item contents" }).textContent)
                .toBe(longBody),
        );
        expect(container.querySelector('[data-mode="reader"]')).toBeNull();
        expect(dialog.querySelector('[data-mode="reader"]')).not.toBeNull();
        expect(within(dialog).getByRole("button", { name: "Pin item" })).toBeTruthy();
        expect(within(dialog).getByRole("button", { name: "Delete item" })).toBeTruthy();
        await user.keyboard("{Escape}");
        await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
        expect(document.activeElement).toBe(openReader);
    });

    it("opens the reader from the compact inspector sheet", async () => {
        viewport.width = 390;
        const { user } = renderScreen();
        await user.click(await screen.findByRole("button", { name: "short preview" }));
        const dialog = await screen.findByRole("dialog", { name: "Clipboard item" });
        expect(dialog.querySelector('[data-slot="inspector-shell"]')).not.toBeNull();
        await user.click(within(dialog).getByRole("button", { name: "Show full contents" }));
        await waitFor(() =>
            expect(within(dialog).getByRole("region", { name: "Item contents" }).textContent)
                .toBe(longBody),
        );
        expect(dialog.querySelector('[data-mode="reader"]')).not.toBeNull();
        expect(dialog.querySelector('[data-slot="dialog-sheet-handle"]')).not.toBeNull();
    });

    it("keeps compact copy pending, consumes failure and leaves safe recovery", async () => {
        viewport.width = 390;
        let failCopy!: (reason: Error) => void;
        ipc.copyItem.mockImplementation(
            () => new Promise<void>((_resolve, reject) => { failCopy = reject; }),
        );
        const { user } = renderScreen();
        await user.click(await screen.findByRole("button", { name: "short preview" }));
        const dialog = await screen.findByRole("dialog", { name: "Clipboard item" });

        await user.click(within(dialog).getByRole("button", { name: "Copy" }));
        await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledOnce());
        expect(within(dialog).getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
            .toBe(true);
        expect(within(dialog).getByRole("button", { name: "Show full contents" }).hasAttribute("disabled"))
            .toBe(true);
        expect(within(dialog).getByRole("button", { name: "Delete item" }).hasAttribute("disabled"))
            .toBe(true);
        await user.keyboard("{Escape}");
        const backdrop = document.querySelector<HTMLElement>('[data-slot="dialog-overlay"]');
        await user.click(backdrop!);
        expect(screen.getByRole("dialog", { name: "Clipboard item" })).toBe(dialog);

        failCopy(new Error("/Users/private/secret.sock"));
        await waitFor(() => expect(toast.error).toHaveBeenCalledOnce());
        expect(String(toast.error.mock.calls[0]?.[0])).not.toContain("/Users/private");
        expect(within(dialog).getByRole("button", { name: "Copy" }).hasAttribute("disabled"))
            .toBe(false);
        await user.click(within(dialog).getByRole("button", { name: "Show full contents" }));
        expect(dialog.querySelector('[data-mode="reader"]')).not.toBeNull();
    });

    it("opens the reader with the desktop list keyboard action", async () => {
        viewport.width = 1200;
        const { user } = renderScreen();
        await screen.findByRole("button", { name: "short preview" });
        const list = screen.getByRole("list", { name: "Clipboard history" });
        list.focus();
        await user.keyboard("{ArrowDown}{ArrowRight}");
        const dialog = await screen.findByRole("dialog", { name: "Clipboard item" });
        expect(dialog.querySelector('[data-mode="reader"]')).not.toBeNull();
    });
});
