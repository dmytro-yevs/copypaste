import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button asChild", () => {
    it("forwards the button surface and keeps a content layout hook", () => {
        render(
            <Button asChild variant="ghost" className="product-link">
                <a href="/releases">
                    <span data-testid="link-layout">Release notes</span>
                </a>
            </Button>,
        );

        const link = screen.getByRole("link", { name: "Release notes" });
        expect(link.getAttribute("href")).toBe("/releases");
        expect(link.getAttribute("data-slot")).toBe("button");
        expect(link.classList.contains("product-link")).toBe(true);
        expect(link.querySelector('[data-slot="button-content"]')).not.toBeNull();
        expect(screen.getByTestId("link-layout")).toBe(link.firstElementChild?.firstElementChild);
    });
});
