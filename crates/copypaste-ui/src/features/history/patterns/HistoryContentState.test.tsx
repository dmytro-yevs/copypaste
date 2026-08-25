import type { ComponentProps } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { HistoryContentState } from "./HistoryContentState";
import { HistoryList } from "./HistoryList";

const emptyList = { items: [] } as unknown as ComponentProps<typeof HistoryList>;

describe("HistoryContentState errors", () => {
    it("shows the repair bot and preserves retry and diagnostics actions", async () => {
        const user = userEvent.setup();
        const retry = vi.fn();
        const diagnostics = vi.fn();
        const { container } = render(
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
});
