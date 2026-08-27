import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import type { ServiceState } from "@/lib/ipc";
import { testClient, withUser } from "@/test/harness";
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
const MATCHING: ServiceState = {
    state: "running",
    version: "2.0.0-alpha.33",
    matches_app: true,
    ours: true,
};

function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });
    return { promise, reject, resolve };
}

beforeEach(() => {
    ipc.serviceState.mockReset().mockResolvedValue(STOPPED);
    ipc.startService.mockReset().mockResolvedValue(MATCHING);
    ipc.restartService.mockReset();
    toastError.mockReset();
});

describe("offline service recovery", () => {
    it("starts only from stopped and blocks an unresolved double click", async () => {
        const pending = deferred<ServiceState>();
        ipc.startService.mockReturnValue(pending.promise);
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );
        const start = await screen.findByRole("button", {
            name: "Start the service",
        });

        await user.dblClick(start);

        expect(ipc.startService).toHaveBeenCalledOnce();
        expect(start.hasAttribute("disabled")).toBe(true);

        pending.resolve(MATCHING);
        await screen.findByText("The clipboard service is running");
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

    it("never offers start while the state query is pending or failed", async () => {
        const pending = deferred<ServiceState>();
        ipc.serviceState.mockReturnValueOnce(pending.promise);
        const first = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );

        expect(await screen.findByText("Checking the clipboard service…")).toBeTruthy();
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();

        first.unmount();
        pending.reject({ code: "unknown", retryable: true });
        ipc.serviceState.mockRejectedValueOnce({
            code: "unknown",
            retryable: true,
            message: "/Users/private/daemon.sock",
        });
        withUser(<ServiceOfflineState onOpenDiagnostics={vi.fn()} />);

        expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
        expect(screen.queryByText(/Users\/private/)).toBeNull();
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();
    });

    it("does not offer to start a build without a bundled service", async () => {
        ipc.serviceState.mockResolvedValue({ state: "not_installed" });
        withUser(<ServiceOfflineState onOpenDiagnostics={vi.fn()} />);

        expect(
            await screen.findByText("This build has no background service"),
        ).toBeTruthy();
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();
    });

    it("offers an adopted mismatched service one guarded restart", async () => {
        const pending = deferred<ServiceState>();
        ipc.serviceState.mockResolvedValue({
            state: "running",
            version: "1.0.0",
            matches_app: false,
            ours: false,
        });
        ipc.restartService.mockReturnValue(pending.promise);
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={vi.fn()} />,
        );
        const restart = await screen.findByRole("button", {
            name: "Restart the service",
        });

        await user.dblClick(restart);

        expect(ipc.restartService).toHaveBeenCalledOnce();
        expect(restart.hasAttribute("disabled")).toBe(true);
        pending.resolve(MATCHING);
        await screen.findByText("The clipboard service is running");
    });

    it("refreshes history and status when a matching service wins the race", async () => {
        ipc.serviceState.mockResolvedValue(MATCHING);
        const client = testClient();
        const invalidate = vi.spyOn(client, "invalidateQueries");
        withUser(<ServiceOfflineState onOpenDiagnostics={vi.fn()} />, client);

        expect(await screen.findByText("The clipboard service is running")).toBeTruthy();
        await waitFor(() => {
            expect(
                invalidate.mock.calls.some(
                    ([filters]) =>
                        JSON.stringify(filters?.queryKey) ===
                        JSON.stringify(["history", "pages"]),
                ),
            ).toBe(true);
            expect(invalidate).toHaveBeenCalledWith({ queryKey: ["status"] });
        });
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();
        expect(await screen.findByRole("button", { name: "Try again" })).toBeTruthy();
    });

    it("keeps an unhealthy live service on diagnostics and state retry", async () => {
        ipc.serviceState
            .mockResolvedValueOnce({ state: "unhealthy" })
            .mockResolvedValueOnce(STOPPED);
        const diagnostics = vi.fn();
        const { user } = withUser(
            <ServiceOfflineState onOpenDiagnostics={diagnostics} />,
        );

        expect(
            await screen.findByText("The clipboard service isn't responding correctly"),
        ).toBeTruthy();
        expect(
            screen.queryByRole("button", { name: "Start the service" }),
        ).toBeNull();
        await user.click(screen.getByRole("button", { name: "Open diagnostics" }));
        expect(diagnostics).toHaveBeenCalledOnce();

        await user.click(screen.getByRole("button", { name: "Try again" }));
        expect(
            await screen.findByRole("button", { name: "Start the service" }),
        ).toBeTruthy();
    });
});
