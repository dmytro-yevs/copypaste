import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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

    it("blocks a pending slotted button before its child handler can run", () => {
        const onClick = vi.fn();
        const onClickCapture = vi.fn();
        render(<Button asChild pending><button type="button" onClick={onClick} onClickCapture={onClickCapture}>Save</button></Button>);

        const button = screen.getByRole("button", { name: "Save" });
        expect(button.getAttribute("aria-disabled")).toBe("true");
        fireEvent.click(button);
        expect(onClick).not.toHaveBeenCalled();
        expect(onClickCapture).not.toHaveBeenCalled();
    });

    it("blocks a disabled slotted link and its keyboard activation", () => {
        const onClick = vi.fn();
        const onKeyDown = vi.fn();
        const onClickCapture = vi.fn();
        const onKeyDownCapture = vi.fn();
        render(<Button asChild disabled><a href="/releases" onClick={onClick} onClickCapture={onClickCapture} onKeyDown={onKeyDown} onKeyDownCapture={onKeyDownCapture}>Releases</a></Button>);

        const link = screen.getByRole("link", { name: "Releases" });
        expect(link.getAttribute("aria-disabled")).toBe("true");
        expect(link.getAttribute("tabindex")).toBe("-1");
        fireEvent.click(link);
        fireEvent.keyDown(link, { key: "Enter" });
        expect(onClick).not.toHaveBeenCalled();
        expect(onClickCapture).not.toHaveBeenCalled();
        expect(onKeyDown).not.toHaveBeenCalled();
        expect(onKeyDownCapture).not.toHaveBeenCalled();
    });

    it("retains child and Button capture handlers when enabled", () => {
        const childCapture = vi.fn();
        const buttonCapture = vi.fn();
        const onClick = vi.fn();
        render(<Button asChild onClickCapture={buttonCapture}><a href="/releases" onClickCapture={childCapture} onClick={(event) => { event.preventDefault(); onClick(); }}>Releases</a></Button>);

        fireEvent.click(screen.getByRole("link", { name: "Releases" }));
        expect(childCapture).toHaveBeenCalledOnce();
        expect(buttonCapture).toHaveBeenCalledOnce();
        expect(onClick).toHaveBeenCalledOnce();
    });
});

describe("Button shared action behavior", () => {
    it("keeps a visible action name when title contains supplementary help", () => {
        render(<Button title="Open capture setup">Set up</Button>);
        expect(screen.getByRole("button", { name: "Set up" }).title).toBe("Open capture setup");
    });
    it("renders a supplied label for a regular action and only names an icon action", () => {
        render(<TooltipProvider><Button icon="copy" label="Copy item" /><Button icon="close" size="icon" label="Close" /></TooltipProvider>);
        expect(screen.getByRole("button", { name: "Copy item" }).textContent).toBe("Copy item");
        expect(screen.getByRole("button", { name: "Close" }).textContent).toBe("");
    });

    it("retains a tooltip hover target around a disabled slotted icon button", () => {
        render(<TooltipProvider><Button asChild size="icon" label="Close" disabled><button>×</button></Button></TooltipProvider>);
        const button = screen.getByRole("button", { name: "Close" });
        expect(button).toHaveProperty("disabled", true);
        expect(button.parentElement?.tagName).toBe("SPAN");
    });

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
