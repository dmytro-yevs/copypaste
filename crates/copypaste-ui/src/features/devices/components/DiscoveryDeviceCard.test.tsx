import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DiscoveryDeviceCard } from "./DiscoveryDeviceCard";

describe("DiscoveryDeviceCard", () => {
    it("uses one details button without a roster pairing action", () => {
        const onSelect = vi.fn();
        render(
            <DiscoveryDeviceCard
                device={{
                    discovery_id: "nearby-1",
                    name: "Studio Mac",
                    addr: "192.0.2.10:47654",
                    last_seen_ms: Date.now(),
                    paired: false,
                }}
                selected={false}
                onSelect={onSelect}
            />,
        );

        const buttons = screen.getAllByRole("button");
        expect(buttons).toHaveLength(1);
        expect(buttons[0].textContent).toContain("Studio Mac");
        expect(buttons[0].textContent).toContain("Nearby · name unverified");
        expect(buttons[0].textContent).not.toContain("Not paired");
        expect(screen.queryByText("Connect")).toBeNull();
        fireEvent.click(buttons[0]);
        expect(onSelect).toHaveBeenCalledTimes(1);
    });
});
