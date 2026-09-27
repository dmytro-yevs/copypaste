import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DiscoveryStage } from "./DiscoveryStage";

describe("DiscoveryStage", () => {
    it("shows an actionable, static unavailable state without a radar", () => {
        const { container } = render(
            <DiscoveryStage state="error" deviceCount={0}>
                <div>Not rendered</div>
            </DiscoveryStage>,
        );

        expect(screen.getByRole("alert").textContent).toContain(
            "Network discovery is unavailable",
        );
        expect(
            screen.getByText("Devices on this network couldn’t be checked."),
        ).toBeTruthy();
        expect(container.querySelector("svg")).toBeTruthy();
        expect(container.querySelector("[data-radar-sweep]")).toBeNull();
        expect(container.querySelector("[data-radar-local-node]")).toBeNull();
    });

    it("keeps discovered devices mounted while a manual refresh is in progress", () => {
        const { rerender } = render(
            <DiscoveryStage state="results" deviceCount={2}>
                <div data-testid="device-list">Nearby devices</div>
            </DiscoveryStage>,
        );

        const list = screen.getByTestId("device-list");
        const summary = screen.getByRole("status");
        expect(summary.textContent).toContain("2 devices found");
        expect(summary.textContent).toContain("Protected pairing starts");

        rerender(
            <DiscoveryStage state="results" deviceCount={2} refreshing>
                <div data-testid="device-list">Nearby devices</div>
            </DiscoveryStage>,
        );

        expect(screen.getByTestId("device-list")).toBe(list);
        expect(screen.getByRole("status").textContent).toContain(
            "Refreshing nearby devices…",
        );
        expect(screen.getByRole("status").parentElement?.getAttribute("aria-busy")).toBe(
            "true",
        );
    });

    it("uses a compact busy state before the first discovery response", () => {
        render(
            <DiscoveryStage state="checking" deviceCount={0}>
                <div>Not rendered</div>
            </DiscoveryStage>,
        );

        expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
        expect(screen.getByText("Checking nearby devices…")).toBeTruthy();
    });
});
