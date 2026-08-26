import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { useRuntimeLog } from "@/features/diagnostics/hooks/useRuntimeLog";
import { RuntimeLogViewer } from "./RuntimeLogViewer";

vi.mock("@/features/diagnostics/hooks/useRuntimeLog", () => ({
    useRuntimeLog: vi.fn(),
}));

const runtimeLogMock = vi.mocked(useRuntimeLog);

beforeEach(() => {
    runtimeLogMock.mockReset();
});

describe("RuntimeLogViewer errors", () => {
    it("keeps the empty result inside the runtime-event surface", () => {
        runtimeLogMock.mockReturnValue({
            rows: [],
            events: [],
            isPending: false,
            isError: false,
            followFailed: false,
            overrun: false,
            hasNextPage: false,
            isFetchingNextPage: false,
            loadOlder: vi.fn(),
            refetch: vi.fn(),
        });

        const { container } = render(
            <TooltipProvider>
                <RuntimeLogViewer />
            </TooltipProvider>,
        );

        expect(screen.getByText("No runtime events match these filters.")).toBeTruthy();
        const empty = container.querySelector("[data-runtime-log-empty]");
        expect(empty).toBeTruthy();
        expect(empty?.querySelector("section")).toBeNull();
    });

    it("uses shared alert notices for feed loss signals", () => {
        runtimeLogMock.mockReturnValue({
            rows: [],
            events: [],
            isPending: false,
            isError: false,
            followFailed: true,
            overrun: true,
            hasNextPage: false,
            isFetchingNextPage: false,
            loadOlder: vi.fn(),
            refetch: vi.fn(),
        });

        render(
            <TooltipProvider>
                <RuntimeLogViewer />
            </TooltipProvider>,
        );

        const alerts = screen.getAllByRole("alert");
        expect(alerts).toHaveLength(2);
        expect(alerts[0]?.textContent).toContain("Events arrived faster");
        expect(alerts[1]?.textContent).toContain("Live updates stopped");
    });

    it("uses the illustrated recovery state and retries the feed", async () => {
        const user = userEvent.setup();
        const refetch = vi.fn();
        runtimeLogMock.mockReturnValue({
            rows: [],
            events: [],
            isPending: false,
            isError: true,
            followFailed: false,
            overrun: false,
            hasNextPage: false,
            isFetchingNextPage: false,
            loadOlder: vi.fn(),
            refetch,
        });

        const { container } = render(
            <TooltipProvider>
                <RuntimeLogViewer />
            </TooltipProvider>,
        );

        const alert = screen.getByRole("alert");
        expect(alert.textContent).toContain("Couldn’t load runtime events");
        expect(alert.textContent).toContain(
            "The diagnostic feed isn’t available right now.",
        );
        expect(container.querySelector('svg[aria-hidden="true"]')).toBeTruthy();

        await user.click(screen.getByRole("button", { name: "Try again" }));
        expect(refetch).toHaveBeenCalledOnce();
    });
});
