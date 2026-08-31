import { useRef, useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { PairingController } from "@/features/pairing/hooks/usePairing";
import { TooltipProvider } from "@/components/ui";
import { IpcFailure } from "@/lib/errors";
import {
    PAIRING_SEMANTICS_BY_STATE,
    type PairingCeremony,
    type PairingState,
} from "@/lib/ipc";
import { PairingLauncherDialog } from "./PairingLauncherDialog";

function ceremony(state: PairingState): PairingCeremony {
    return {
        ceremony_id: "preview-ceremony",
        role: "initiator",
        state,
        semantics: PAIRING_SEMANTICS_BY_STATE[state],
        presentation: "unavailable",
        known_device:
            state === "confirmed"
                ? {
                      name: "Studio Mac",
                      last_seen_ms: Date.now(),
                      online: true,
                  }
                : null,
        error: null,
    };
}

const waiting = ceremony("waiting_for_peer");
const connecting = ceremony("handshaking");
const confirmed = ceremony("confirmed");
const protectedInvite = {
    code: "482 916",
    listen_addr: "192.168.1.20:49200",
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
        decisionSubmitted: null,
        canRetry: false,
        startPreviewCreate: vi.fn(),
        run: vi.fn(),
        retry: vi.fn(),
        ...overrides,
    } as PairingController;
}

function launcher(
    pairing: PairingController,
    onOpenChange: (open: boolean) => void = vi.fn(),
    preview = true,
    onCreate = vi.fn(),
    onJoin = vi.fn(),
) {
    return (
        <TooltipProvider>
            <PairingLauncherDialog
                open
                available
                preview={preview}
                disabled={false}
                pairing={pairing}
                onOpenChange={onOpenChange}
                onCreate={onCreate}
                onJoin={onJoin}
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
    it("keeps Android backdrop dismissal and scanner entry reachable", async () => {
        const pairing = controller();
        const onOpenChange = vi.fn();
        const user = userEvent.setup();
        const url = window.location.href;
        window.history.replaceState({}, "", "/?platform=android");
        document.body.id = "application-shell";
        try {
            const { unmount } = render(launcher(pairing, onOpenChange));

            const overlay = document.querySelector<HTMLElement>(
                '[data-slot="dialog-overlay"]',
            );
            expect(overlay).toBeTruthy();
            expect(overlay?.getAttribute("aria-label")).toBeNull();
            expect(overlay?.getAttribute("role")).toBeNull();
            expect(overlay?.style.pointerEvents).not.toBe("none");
            expect(document.body.getAttribute("aria-label")).toBeNull();
            expect(document.body.id).toBe("copypaste-pairing-dialog-open");
            expect(
                screen.getByRole("button", { name: /Scan pairing code/ }),
            ).toBeTruthy();

            await user.click(overlay as HTMLElement);
            expect(onOpenChange).toHaveBeenCalledWith(false);

            unmount();
            expect(document.body.id).toBe("application-shell");
        } finally {
            document.body.removeAttribute("id");
            window.history.replaceState({}, "", url);
        }
    });

    it("removes the Android pairing marker when the body had no id", () => {
        const url = window.location.href;
        window.history.replaceState({}, "", "/?platform=android");
        document.body.removeAttribute("id");
        try {
            const { unmount } = render(launcher(controller(), vi.fn()));

            expect(document.body.id).toBe("copypaste-pairing-dialog-open");
            unmount();
            expect(document.body.getAttribute("id")).toBeNull();
        } finally {
            document.body.removeAttribute("id");
            window.history.replaceState({}, "", url);
        }
    });

    it.each(["Escape", "close button", "backdrop"])(
        "cancels an active ceremony and restores focus after %s dismissal",
        async (dismissal) => {
            const pairing = controller({ ceremony: waiting });
            const user = userEvent.setup();
            render(<StatefulLauncher pairing={pairing} />);

            if (dismissal === "Escape") {
                fireEvent.keyDown(screen.getByRole("dialog"), {
                    key: "Escape",
                });
            } else if (dismissal === "close button") {
                await user.click(screen.getByRole("button", { name: "Close" }));
            } else {
                await user.click(
                    document.querySelector<HTMLElement>(
                        '[data-slot="dialog-overlay"]',
                    ) as HTMLElement,
                );
            }

            expect(pairing.run).toHaveBeenCalledWith("cancel");
            await waitFor(() => {
                expect(document.activeElement?.textContent).toBe(
                    "Launch pairing",
                );
            });
        },
    );

    it("does not repeat a cancellation that is already pending", async () => {
        const pairing = controller({
            ceremony: waiting,
            isPending: true,
            pendingAction: "cancel",
        });
        const onOpenChange = vi.fn();
        render(launcher(pairing, onOpenChange));

        fireEvent.click(
            screen.getByRole("button", { name: /Show pairing code/ }),
        );

        expect(
            (screen.getByRole("button", {
                name: "Cancelling…",
            }) as HTMLButtonElement).disabled,
        ).toBe(true);
        await userEvent.setup().click(screen.getByRole("button", { name: "Close" }));

        expect(pairing.run).not.toHaveBeenCalled();
        expect(onOpenChange).toHaveBeenCalledWith(false);
    });

    it.each(["Escape", "close button", "backdrop"])(
        "cancels an active ceremony despite a client error after %s dismissal",
        async (dismissal) => {
            const pairing = controller({
                ceremony: waiting,
                error: new IpcFailure("peer_unreachable", true),
            });
            const user = userEvent.setup();
            render(<StatefulLauncher pairing={pairing} />);

            if (dismissal === "Escape") {
                fireEvent.keyDown(screen.getByRole("dialog"), {
                    key: "Escape",
                });
            } else if (dismissal === "close button") {
                await user.click(screen.getByRole("button", { name: "Close" }));
            } else {
                await user.click(
                    document.querySelector<HTMLElement>(
                        '[data-slot="dialog-overlay"]',
                    ) as HTMLElement,
                );
            }

            expect(pairing.run).toHaveBeenCalledWith("cancel");
            await waitFor(() => {
                expect(document.activeElement?.textContent).toBe(
                    "Launch pairing",
                );
            });
        },
    );

    it("keeps preview pairing material out of DOM text, values, and attributes", () => {
        const pairing = {
            ...controller({ ceremony: waiting }),
            previewInvite: protectedInvite,
        } as PairingController;
        const { baseElement } = render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: /Show pairing code/ }),
        );

        expect(pairing.startPreviewCreate).toHaveBeenCalledTimes(1);
        expect(baseElement.querySelector("img")).toBeNull();
        const forbidden = [
            ...Object.values(protectedInvite),
            encodeURIComponent(protectedInvite.qr_svg),
        ];
        for (const secret of forbidden) {
            expect(baseElement.textContent).not.toContain(secret);
            for (const element of baseElement.querySelectorAll<HTMLElement>("*")) {
                for (const name of element.getAttributeNames()) {
                    expect(element.getAttribute(name)).not.toContain(secret);
                }
                if (element instanceof HTMLInputElement) {
                    expect(element.value).not.toContain(secret);
                }
            }
        }
    });

    it("renders semantic progress without a web code-entry path", () => {
        const pairing = controller({ ceremony: connecting });
        render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: "Enter pairing code" }),
        );

        expect(
            screen.getByText("Establishing a private connection…"),
        ).toBeTruthy();
        expect(screen.queryByRole("textbox")).toBeNull();
        expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
        expect(pairing.run).not.toHaveBeenCalled();
    });

    it.each([
        ["Show pairing code", "create"],
        ["Enter pairing code", "join"],
    ] as const)(
        "keeps native %s callbacks available outside preview",
        async (label, action) => {
            const onOpenChange = vi.fn();
            const onCreate = vi.fn();
            const onJoin = vi.fn();
            const pairing = controller();
            render(launcher(pairing, onOpenChange, false, onCreate, onJoin));

            await userEvent.setup().click(
                screen.getByRole("button", { name: label }),
            );

            expect(onOpenChange).toHaveBeenCalledWith(false);
            expect(onCreate).toHaveBeenCalledTimes(action === "create" ? 1 : 0);
            expect(onJoin).toHaveBeenCalledTimes(action === "join" ? 1 : 0);
            expect(pairing.startPreviewCreate).not.toHaveBeenCalled();
        },
    );

    it.each([
        ["confirmed", "Done", false],
        ["rejected", "Try again", true],
        ["cancelled", "Try again", true],
        ["timed_out", "Try again", true],
        ["failed", "Back", false],
    ] as const)(
        "uses generated terminal semantics for %s",
        (state, action, canRetry) => {
            const retry = vi.fn();
            const pairing = controller({
                ceremony: ceremony(state),
                canRetry,
                retry,
            });
            render(launcher(pairing));

            fireEvent.click(
                screen.getByRole("button", { name: /Show pairing code/ }),
            );
            fireEvent.click(screen.getByRole("button", { name: action }));

            expect(retry).toHaveBeenCalledTimes(action === "Try again" ? 1 : 0);
        },
    );

    it.each([
        [new IpcFailure("content_too_large", true), "Back"],
        [new IpcFailure("future_error", true), "Back"],
        [new IpcFailure("peer_unreachable", true), "Try again"],
    ] as const)(
        "uses canonical client-error retry policy",
        (error, action) => {
            const retry = vi.fn();
            const pairing = controller({
                ceremony: waiting,
                error,
                canRetry: true,
                retry,
            });
            render(launcher(pairing));

            fireEvent.click(
                screen.getByRole("button", { name: /Show pairing code/ }),
            );
            fireEvent.click(screen.getByRole("button", { name: action }));

            expect(retry).toHaveBeenCalledTimes(action === "Try again" ? 1 : 0);
        },
    );

    it("keeps successful semantic details meaningful in the safe preview", () => {
        const pairing = controller({ ceremony: confirmed });
        render(launcher(pairing));

        fireEvent.click(
            screen.getByRole("button", { name: /Show pairing code/ }),
        );

        expect(screen.getByText("Device paired")).toBeTruthy();
        expect(screen.getByText("Studio Mac is ready to sync.")).toBeTruthy();
    });
});
