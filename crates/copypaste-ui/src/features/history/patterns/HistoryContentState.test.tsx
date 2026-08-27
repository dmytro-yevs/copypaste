import type { ComponentProps } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { item } from "@/test/harness";
import { HistoryContentState } from "./HistoryContentState";
import { HistoryList } from "./HistoryList";

vi.mock("@/features/history/patterns/HistoryList", () => ({
    HistoryList: ({
        items,
    }: {
        items: readonly { content: string | null }[];
    }) => (
        <div role="list">
            {items.map((entry, index) => (
                <span key={index}>{entry.content}</span>
            ))}
        </div>
    ),
}));

const emptyList = { items: [] } as unknown as ComponentProps<typeof HistoryList>;
const cachedList = {
    items: [item({ content: "cached clipboard entry" })],
} as unknown as ComponentProps<typeof HistoryList>;

describe("HistoryContentState errors", () => {
    it("shows the repair bot and preserves retry and diagnostics actions", async () => {
        const user = userEvent.setup();
        const retry = vi.fn();
        const diagnostics = vi.fn();
        const { container } = render(
            <HistoryContentState
                loading={false}
                errorKind="timeout"
                searching={false}
                filtered={false}
                privateMode={false}
                capturePaused={false}
                query=""
                hasMore={false}
                onLoadMore={vi.fn()}
                onRetry={retry}
                onOpenCapture={vi.fn()}
                onOpenDiagnostics={diagnostics}
                list={emptyList}
            />,
        );

        expect(screen.getByRole("alert").textContent).toContain(
            "Failed to load history",
        );
        expect(container.querySelector('svg[aria-hidden="true"]')).toBeTruthy();

        await user.click(screen.getByRole("button", { name: "Try again" }));
        await user.click(
            screen.getByRole("button", { name: "Open diagnostics" }),
        );
        expect(retry).toHaveBeenCalledOnce();
        expect(diagnostics).toHaveBeenCalledOnce();
    });

    it("keeps cached rows visible while disclosing an offline service", async () => {
        const user = userEvent.setup();
        const retry = vi.fn();
        const diagnostics = vi.fn();

        render(
            <HistoryContentState
                loading={false}
                errorKind="offline"
                searching={false}
                filtered={false}
                privateMode={false}
                capturePaused={false}
                query=""
                hasMore={false}
                onLoadMore={vi.fn()}
                onRetry={retry}
                onOpenCapture={vi.fn()}
                onOpenDiagnostics={diagnostics}
                list={cachedList}
            />,
        );

        expect(screen.getByRole("list").textContent).toContain(
            "cached clipboard entry",
        );
        expect(screen.getByRole("status").textContent).toContain(
            "The clipboard service isn't running",
        );

        await user.click(screen.getByRole("button", { name: "Try again" }));
        await user.click(
            screen.getByRole("button", { name: "Open diagnostics" }),
        );
        expect(retry).toHaveBeenCalledOnce();
        expect(diagnostics).toHaveBeenCalledOnce();
    });
});

describe("HistoryContentState empty history", () => {
    it("explains that the service returned no clipboard items", () => {
        render(
            <HistoryContentState
                loading={false}
                errorKind={null}
                searching={false}
                filtered={false}
                privateMode={false}
                capturePaused={false}
                query=""
                hasMore={false}
                onLoadMore={vi.fn()}
                onRetry={vi.fn()}
                onOpenCapture={vi.fn()}
                onOpenDiagnostics={vi.fn()}
                list={emptyList}
            />,
        );

        const status = screen.getByRole("status");
        expect(status.textContent).toContain("Nothing copied yet");
        expect(status.textContent).toContain(
            "Copy something and it will appear here.",
        );
    });
});
