import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Button } from "@/components/ui";
import { IllustratedErrorState } from "./IllustratedErrorState";

describe("IllustratedErrorState", () => {
    it("announces the error, keeps the repair bot decorative, and exposes recovery actions", async () => {
        const user = userEvent.setup();
        const retry = vi.fn();
        const diagnostics = vi.fn();
        const { container } = render(
            <IllustratedErrorState
                title="History needs attention"
                body="The service did not answer."
                actions={
                    <>
                        <Button onClick={retry}>Try again</Button>
                        <Button onClick={diagnostics}>Open diagnostics</Button>
                    </>
                }
            />,
        );

        const alert = screen.getByRole("alert");
        expect(alert.textContent).toContain("History needs attention");
        expect(alert.textContent).toContain("The service did not answer.");
        expect(
            container
                .querySelector('svg[aria-hidden="true"]')
                ?.getAttribute("focusable"),
        ).toBe("false");

        await user.click(screen.getByRole("button", { name: "Try again" }));
        await user.click(
            screen.getByRole("button", { name: "Open diagnostics" }),
        );
        expect(retry).toHaveBeenCalledOnce();
        expect(diagnostics).toHaveBeenCalledOnce();
    });
});
