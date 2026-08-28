import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { DiscoveryPairingFooter } from "./DiscoveryPairingFooter";

function controller(
    overrides: Partial<PairingController> = {},
): PairingController {
    return {
        webPreview: true,
        protectedPresentationAvailable: false,
        ceremony: undefined,
        error: null,
        isChecking: false,
        isPending: false,
        pendingAction: undefined,
        lastAttempt: null,
        presentation: null,
        previewInvite: null,
        decisionSubmitted: null,
        canRetry: false,
        clearPreviewInvite: vi.fn(),
        startPreviewCreate: vi.fn(),
        submitPreviewJoin: vi.fn(),
        run: vi.fn(),
        retry: vi.fn(),
        ...overrides,
    } as PairingController;
}

describe("DiscoveryPairingFooter", () => {
    it("keeps Connect as the unpaired drawer primary action", () => {
        const onConnect = vi.fn();
        render(
            <DiscoveryPairingFooter
                deviceName="Studio Mac"
                state="idle"
                disabled={false}
                pairing={controller()}
                onConnect={onConnect}
            />,
        );

        fireEvent.click(screen.getByRole("button", { name: /Connect to Studio Mac/ }));
        expect(onConnect).toHaveBeenCalledTimes(1);
    });

    it("switches the sticky action through cancel, retry, and paired states", () => {
        const pairing = controller({
            ceremony: {
                ceremony_id: "ceremony-1",
                role: "initiator",
                state: "handshaking",
                semantics: {
                    message_id: "securing_connection",
                    icon: "spinner",
                    tone: "info",
                    live: "status",
                    active: true,
                    terminal: false,
                    needs_devices: true,
                    review_secure: false,
                    retry: false,
                },
                presentation: "presented",
                known_device: null,
                error: null,
            },
        });
        const { rerender } = render(
            <DiscoveryPairingFooter
                deviceName="Studio Mac"
                state="pending"
                disabled={false}
                pairing={pairing}
                onConnect={vi.fn()}
            />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        expect(pairing.run).toHaveBeenCalledWith("cancel");

        const retry = vi.fn();
        rerender(
            <DiscoveryPairingFooter
                deviceName="Studio Mac"
                state="error"
                disabled={false}
                pairing={controller({ error: new Error("failed") })}
                onConnect={retry}
            />,
        );
        fireEvent.click(screen.getByRole("button", { name: "Try again" }));
        expect(retry).toHaveBeenCalledTimes(1);

        rerender(
            <DiscoveryPairingFooter
                deviceName="Studio Mac"
                state="success"
                disabled={false}
                pairing={controller()}
                onConnect={vi.fn()}
            />,
        );
        expect(screen.getByRole("status").textContent).toContain("Paired");
        expect(screen.queryByRole("button")).toBeNull();
    });
});
