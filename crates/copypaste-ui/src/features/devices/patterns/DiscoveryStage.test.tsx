import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DiscoveryStage } from "./DiscoveryStage";

describe("DiscoveryStage radar", () => {
    it("replaces the active radar with a static unavailable state", () => {
        const { container } = render(
            <DiscoveryStage state="error" devices={[]}>
                <div>Not rendered</div>
            </DiscoveryStage>,
        );

        expect(screen.getByRole("alert").textContent).toContain(
            "Network discovery is unavailable",
        );
        expect(
            screen.getByText("Devices on this network couldn’t be checked."),
        ).toBeTruthy();
        expect(
            container.querySelector("[data-discovery-unavailable]"),
        ).toBeTruthy();
        expect(container.querySelector("[data-radar-sweep]")).toBeNull();
        expect(container.querySelector("[data-radar-local-node]")).toBeNull();
    });

    it("maps supplied devices to labeled radar cards and latency bands", () => {
        const onSelect = vi.fn();
        const { container, rerender } = render(
            <DiscoveryStage
                state="scanning"
                devices={[
                    {
                        id: "device-macos",
                        name: "Studio Mac",
                        identity: {
                            platform: "macos",
                            formFactor: "laptop",
                            source: "peer-asserted",
                        },
                        address: "192.0.2.10",
                        status: "Available to pair",
                        latencyMs: 12,
                        paired: false,
                        selected: false,
                        onSelect,
                    },
                    {
                        id: "device-windows",
                        name: "Work PC",
                        identity: {
                            platform: "windows",
                            formFactor: "desktop",
                            source: "peer-asserted",
                        },
                        address: "192.0.2.11",
                        status: "Available to pair",
                        latencyMs: 36,
                        paired: false,
                        selected: false,
                        onSelect,
                    },
                    {
                        id: "device-android",
                        name: "Pixel",
                        identity: {
                            platform: "android",
                            formFactor: "phone",
                            source: "peer-asserted",
                        },
                        address: "192.0.2.12",
                        status: "Already paired",
                        latencyMs: 120,
                        paired: true,
                        selected: true,
                        onSelect,
                    },
                    {
                        id: "device-unknown",
                        name: "Nearby device",
                        identity: {
                            platform: "unknown",
                            formFactor: "unknown",
                            source: "unknown",
                        },
                        address: "192.0.2.13",
                        status: "Available to pair",
                        paired: false,
                        selected: false,
                        onSelect,
                    },
                ]}
            >
                <div>Real device card</div>
            </DiscoveryStage>,
        );

        expect(screen.getByLabelText("Nearby device radar")).toBeTruthy();
        fireEvent.click(
            screen.getByRole("button", {
                name: /Studio Mac.*macOS.*192\.0\.2\.10.*12 milliseconds.*Available to pair/,
            }),
        );
        expect(onSelect).toHaveBeenCalledTimes(1);
        expect(screen.getByText("Real device card")).toBeTruthy();
        const workPc = screen.getByRole("button", {
            name: /Work PC.*Windows.*192\.0\.2\.11.*36 milliseconds.*Available to pair/,
        });
        expect(workPc.textContent).toBe("Work PC");
        expect(screen.getByText("Pixel")).toBeTruthy();
        expect(screen.getByText("Nearby device")).toBeTruthy();
        expect(container.textContent).not.toContain("192.0.2.11");
        expect(container.textContent).not.toContain("36 ms");
        expect(container.textContent).not.toContain("Available to pair");
        expect(
            container.querySelectorAll('[data-distance="near"]'),
        ).toHaveLength(1);
        expect(
            container.querySelectorAll('[data-distance="middle"]'),
        ).toHaveLength(1);
        expect(
            container.querySelectorAll('[data-distance="far"]'),
        ).toHaveLength(1);
        expect(
            container.querySelectorAll('[data-distance="unknown"]'),
        ).toHaveLength(1);

        const sweep = container.querySelector("[data-radar-sweep]");
        const localNode = container.querySelector("[data-radar-local-node]");
        expect(sweep).toBeTruthy();
        expect(localNode).toBeTruthy();
        expect(sweep?.children).toHaveLength(1);
        expect(
            sweep?.querySelector("[data-radar-sweep-sector]"),
        ).toBeTruthy();
        expect(container.querySelector("[class*='echoes']")).toBeNull();

        rerender(
            <DiscoveryStage state="results" devices={[]}>
                <div>Not rendered</div>
            </DiscoveryStage>,
        );
        expect(container.querySelector("[data-radar-sweep]")).toBeNull();
        expect(container.querySelector("[data-radar-local-node]")).toBe(
            localNode,
        );

        rerender(
            <DiscoveryStage state="idle" devices={[]}>
                <div>Not rendered</div>
            </DiscoveryStage>,
        );
        expect(screen.queryByRole("button")).toBeNull();
        expect(screen.queryByText("Not rendered")).toBeNull();
    });
});
