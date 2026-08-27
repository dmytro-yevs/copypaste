import { useRef, useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { TooltipProvider } from "@/components/ui";
import type { PairingCeremony, PreviewPairingInvite } from "@/lib/ipc";
import { PairingLauncherDialog } from "./PairingLauncherDialog";

const waiting: PairingCeremony = {
    ceremony_id: "preview-ceremony",
    role: "initiator",
    state: "waiting_for_peer",
    presentation: "presented",
    known_device: null,
    error: null,
};
const connecting: PairingCeremony = {
    ...waiting,
    state: "handshaking",
};
const confirmed: PairingCeremony = {
    ...waiting,
    state: "confirmed",
    known_device: {
        name: "Studio Mac",
        last_seen_ms: Date.now(),
        online: true,
    },
};
const invite: PreviewPairingInvite = {
    ceremony: waiting,
    code: "482 916",
    listen_addr: "192.168.1.20:49200",
    expires_in_secs: 120,
    qr_svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><path d="M0 0h8v8H0z"/></svg>',
};

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
        submitPreviewJoin: vi.fn().mockResolvedValue(connecting),
        run: vi.fn(),
        retry: vi.fn(),
        ...overrides,
    } as PairingController;
}

function launcher(
    pairing: PairingController,
    onOpenChange: (open: boolean) => void = vi.fn(),
) {
    return (
        <TooltipProvider>
            <PairingLauncherDialog
                open
                available
                preview
                disabled={false}
                pairing={pairing}
                onOpenChange={onOpenChange}
                onCreate={vi.fn()}
                onJoin={vi.fn()}
            />
        </TooltipProvider>
    );
}

function StatefulLauncher({ pairing }: { pairing: PairingController }) {
    const [open, setOpen] = useState(true);
    const returnFocusRef = useRef<HTMLButtonElement>(null);

    return (
        <TooltipProvider>
            <button ref={returnFocusRef}>Launch pairing</button>
            <PairingLauncherDialog
                open={open}
                available
                preview
                disabled={false}
                pairing={pairing}
                onOpenChange={setOpen}
                onCreate={vi.fn()}
                onJoin={vi.fn()}
                returnFocusRef={returnFocusRef}
            />
        </TooltipProvider>
    );
}

describe("PairingLauncherDialog preview flows", () => {
    it("keeps the Android overlay inert and scanner entry reachable", () => {
        const pairing = controller();
        const url = window.location.href;
        window.history.replaceState({}, "", "/?platform=android");
        try {
            render(launcher(pairing));

            const overlay = document.querySelector<HTMLElement>(
                '[data-slot="dialog-overlay"]',
            );
            expect(overlay).toBeTruthy();
            expect(overlay?.getAttribute("aria-label")).toBeNull();
            expect(overlay?.getAttribute("role")).toBeNull();
            expect(overlay?.style.pointerEvents).toBe("none");
            expect(
                screen.getByRole("button", { name: /Scan pairing code/ }),
            ).toBeTruthy();
        } finally {
            window.history.replaceState({}, "", url);
        }
    });

    it("preserves default backdrop dismissal outside Android", async () => {
        const onOpenChange = vi.fn();
        const user = userEvent.setup();
        render(launcher(controller(), onOpenChange));

        const overlay = document.querySelector<HTMLElement>(
            '[data-slot="dialog-overlay"]',
        );
        expect(overlay).toBeTruthy();
        expect(overlay?.style.pointerEvents).not.toBe("none");
        await user.click(overlay as HTMLElement);

        expect(onOpenChange).toHaveBeenCalledWith(false);
    });

    it.each(["Escape", "close button"])(
        "cancels an active ceremony and restores focus after %s dismissal",
        async (dismissal) => {
            const pairing = controller({ ceremony: waiting });
            render(<StatefulLauncher pairing={pairing} />);

            if (dismissal === "Escape") {
                fireEvent.keyDown(screen.getByRole("dialog"), {
                    key: "Escape",
                });
            } else {
                fireEvent.click(screen.getByRole("button", { name: "Close" }));
            }

            expect(pairing.run).toHaveBeenCalledWith("cancel");
            await waitFor(() => {
                expect(document.activeElement?.textContent).toBe(
                    "Launch pairing",
                );
            });
        },
    );

    it("opens a host flow with QR, short code, address, and waiting state", () => {
        const pairing = controller();
        const { rerender } = render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: /Show pairing code/ }),
        );
        expect(pairing.startPreviewCreate).toHaveBeenCalledTimes(1);
        expect(screen.queryByLabelText("Pairing code")).toBeNull();

        rerender(
            launcher(controller({ ceremony: waiting, previewInvite: invite })),
        );
        expect(
            screen.getByRole("img", { name: "Pairing QR code" }),
        ).toBeTruthy();
        expect(screen.getByText("482 916")).toBeTruthy();
        expect(screen.getByText("192.168.1.20:49200")).toBeTruthy();
        expect(screen.getByText("Waiting for the other device…")).toBeTruthy();
        expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();
    });

    it("announces a clipboard failure with shared field feedback", async () => {
        const originalClipboard = navigator.clipboard;
        Object.defineProperty(navigator, "clipboard", {
            configurable: true,
            value: {
                writeText: vi.fn().mockRejectedValue(new Error("denied")),
            },
        });
        const pairing = controller();
        const { rerender } = render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: /Show pairing code/ }),
        );
        rerender(
            launcher(controller({ ceremony: waiting, previewInvite: invite })),
        );
        fireEvent.click(
            screen.getByRole("button", { name: "Copy code and address" }),
        );

        await waitFor(() => {
            expect(screen.getByRole("alert").textContent).toContain(
                "Couldn’t copy",
            );
        });
        Object.defineProperty(navigator, "clipboard", {
            configurable: true,
            value: originalClipboard,
        });
    });

    it("validates join input and reaches connecting and success states", () => {
        const pairing = controller();
        const { rerender } = render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: "Enter pairing code" }),
        );
        const connect = screen.getByRole("button", { name: "Connect" });
        expect((connect as HTMLButtonElement).disabled).toBe(true);
        fireEvent.change(screen.getByLabelText("Pairing code"), {
            target: { value: "482 916" },
        });
        fireEvent.change(screen.getByLabelText("Address"), {
            target: { value: "192.168.1.20:49200" },
        });
        expect((connect as HTMLButtonElement).disabled).toBe(false);
        fireEvent.click(connect);
        expect(pairing.submitPreviewJoin).toHaveBeenCalledWith(
            "482916",
            "192.168.1.20:49200",
        );

        rerender(launcher(controller({ ceremony: connecting })));
        expect(
            screen.getByText("Establishing a private connection…"),
        ).toBeTruthy();

        rerender(launcher(controller({ ceremony: confirmed })));
        expect(screen.getByText("Device paired")).toBeTruthy();
        expect(screen.getByText("Studio Mac is ready to sync.")).toBeTruthy();
    });
});
