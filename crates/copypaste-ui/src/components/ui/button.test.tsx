import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";
import { TooltipProvider } from "./tooltip";

const styles = readFileSync(resolve(process.cwd(), "src/components/ui/button.module.css"), "utf8");

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

    it("uses the tap token in compact button sizing without a media-only override", () => {
        const compactSize = "max(var(--ctl-h-sm), var(--tap-min))";
        const compact = styles.match(/\.compact\s*\{[\s\S]*?\}/)?.[0];
        const compactIcon = styles.match(/\.compactIcon\s*\{[\s\S]*?\}/)?.[0];

        expect(compact).toContain(`min-block-size: ${compactSize};`);
        expect(compactIcon).toContain(compactSize);
        expect(compactIcon?.match(/max\(var\(--ctl-h-sm\), var\(--tap-min\)\)/g)).toHaveLength(4);
        expect(styles).not.toMatch(/@media \(pointer: coarse\)/);
    });
});

describe("Button shared action behavior", () => {
    it("names an icon action and disables it while pending with the shared loading visual", () => {
        render(<TooltipProvider><Button icon="copy" label="Copy item" size="icon" pending /></TooltipProvider>);

        const button = screen.getByRole("button", { name: "Copy item" });
        expect(button).toHaveProperty("disabled", true);
        expect(button.getAttribute("aria-busy")).toBe("true");
        expect(button.querySelector('[data-mode="loading"][data-placement="control"]')).not.toBeNull();
    });

    it("keeps a disabled icon action named inside its tooltip trigger", () => {
        render(<TooltipProvider><Button icon="copy" label="Copy item" size="compactIcon" disabled /></TooltipProvider>);

        const button = screen.getByRole("button", { name: "Copy item" });
        expect(button).toHaveProperty("disabled", true);
        expect(button.parentElement?.tagName).toBe("SPAN");
    });
});
