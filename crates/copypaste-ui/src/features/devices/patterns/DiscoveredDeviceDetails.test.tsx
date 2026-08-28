import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DiscoveredDevice } from "@/lib/ipc";
import { DiscoveredDeviceDetails } from "./DiscoveredDeviceDetails";

function device(state: "online" | "offline" | "unknown"): DiscoveredDevice {
    return {
        discovery_id: "nearby-phone",
        name: "Nearby phone",
        addr: "192.0.2.24:47654",
        last_seen_ms: 100,
        paired: false,
        details: {
            profile: null,
            endpoint: null,
            latency: null,
            presence: {
                state,
                last_seen_ms: 100,
                provenance: "observed",
                trust: "local",
                observed_at_ms: 100,
                fresh_until_ms: 200,
            },
            public_ip: { availability: "unavailable" },
            geo: { availability: "unavailable" },
        },
    };
}

describe("DiscoveredDeviceDetails", () => {
    afterEach(() => vi.restoreAllMocks());

    it.each([
        ["online", "Seen on this network"],
        ["offline", "Not currently discovered"],
        ["unknown", "Not available"],
    ] as const)("renders %s presence distinctly", (state, label) => {
        vi.spyOn(Date, "now").mockReturnValue(150);
        const { container } = render(
            <DiscoveredDeviceDetails
                device={device(state)}
                status={{
                    icon: "circle",
                    label: "Not paired",
                    tone: "neutral",
                    busy: false,
                    live: "off",
                }}
            />,
        );

        expect(container.textContent).toContain(label);
    });
});
