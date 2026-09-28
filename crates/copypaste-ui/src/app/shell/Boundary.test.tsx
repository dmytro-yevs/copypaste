import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Boundary } from "./Boundary";

function BrokenRegion(): never {
    throw new Error("render failed");
}

afterEach(() => vi.restoreAllMocks());

describe("Boundary recovery", () => {
    it("offers a focused compact recovery path", () => {
        vi.spyOn(console, "error").mockImplementation(() => undefined);
        const { container } = render(
            <Boundary label="Connections">
                <BrokenRegion />
            </Boundary>,
        );

        expect(screen.getByRole("alert")).toBeTruthy();
        expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
        expect(
            screen.getByRole("button", { name: "Open diagnostics" }),
        ).toBeTruthy();
        expect(container.querySelector('svg[aria-hidden="true"]')).toBeNull();
        expect(screen.getByRole("alert").textContent).toContain(
            "This view is temporarily unavailable, while other CopyPaste screens remain available.",
        );
    });
});
