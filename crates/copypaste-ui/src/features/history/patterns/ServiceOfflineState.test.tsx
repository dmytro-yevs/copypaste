import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import type { ServiceState } from "@/lib/ipc";
import { withUser } from "@/test/harness";
import { ServiceOfflineState } from "./ServiceOfflineState";

const ipc = vi.hoisted(() => ({
    serviceState: vi.fn(),
    startService: vi.fn(),
    restartService: vi.fn(),
}));
const toastError = vi.hoisted(() => vi.fn());

vi.mock("sonner", () => ({ toast: { error: toastError } }));
vi.mock("@/lib/ipc", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/lib/ipc")>()),
    serviceState: () => ipc.serviceState(),
    startService: () => ipc.startService(),
    restartService: () => ipc.restartService(),
}));

const STOPPED: ServiceState = { state: "stopped" };

beforeEach(() => {
    ipc.serviceState.mockReset().mockResolvedValue(STOPPED);
    ipc.startService.mockReset().mockResolvedValue({
        state: "running",
        version: "2.0.0-alpha.33",
        matches_app: true,
        ours: true,
    });
    ipc.restartService.mockReset();
    toastError.mockReset();
});

describe("offline service recovery", () => {
    it("starts the service from the stopped state", async () => {
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );

        await user.click(
            await screen.findByRole("button", { name: "Start the service" }),
        );

        await waitFor(() => expect(ipc.startService).toHaveBeenCalledOnce());
    });

    it("keeps the start action available after a failed attempt", async () => {
        ipc.startService.mockRejectedValue({ code: "offline", retryable: true });
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );
        const start = await screen.findByRole("button", {
            name: "Start the service",
        });

        await user.click(start);

        await waitFor(() => expect(toastError).toHaveBeenCalledOnce());
        expect(start.hasAttribute("disabled")).toBe(false);
    });

    it("does not offer to start a build without a bundled service", async () => {
        ipc.serviceState.mockResolvedValue({ state: "not_installed" });
        withUser(<ServiceOfflineState onOpenDiagnostics={vi.fn()} />);

        expect(await screen.findByText("This build has no background service")).toBeTruthy();
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();
    });

    it("offers an owned mismatched service a restart", async () => {
        ipc.serviceState.mockResolvedValue({
            state: "running",
            version: "1.0.0",
            matches_app: false,
            ours: true,
        });
        ipc.restartService.mockResolvedValue({
            state: "running",
            version: "2.0.0-alpha.33",
            matches_app: true,
            ours: true,
        });
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );

        await user.click(
            await screen.findByRole("button", { name: "Restart the service" }),
        );

        expect(ipc.restartService).toHaveBeenCalledOnce();
    });
});
