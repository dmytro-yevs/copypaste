import { readFileSync } from "node:fs";

import { render, screen } from "@testing-library/react";
import postcss from "postcss";
import { describe, expect, it } from "vitest";

import { InspectorColorPreview } from "./InspectorColorPreview";

describe("InspectorColorPreview", () => {
    it("shows the trimmed hex value as an accessible color graphic", () => {
        const { container } = render(
            <InspectorColorPreview label="Color" value="  #0A84FF  " />,
        );

        expect(
            screen.getByRole("img", { name: "Color: #0A84FF" }),
        ).toBeTruthy();
        expect(container.querySelector("figcaption")?.textContent).toBe(
            "#0A84FF",
        );
        expect(
            container.querySelector<HTMLElement>(
                '[data-slot="inspector-color-field"]',
            )?.style.backgroundColor,
        ).toBe("rgb(10, 132, 255)");
    });

    it("keeps the responsive swatch square", () => {
        const sheet = postcss.parse(
            readFileSync(
                "src/features/history/components/InspectorColorPreview.module.css",
                "utf8",
            ),
        );
        const declarations = new Map<string, string>();
        sheet.walkRules(".swatch", (rule) => {
            if (rule.selector !== ".swatch") return;
            rule.walkDecls((declaration) => {
                declarations.set(declaration.prop, declaration.value);
            });
        });

        expect(declarations.get("inline-size")).toBe("100%");
        expect(declarations.get("max-inline-size")).toBe(
            "calc(var(--s-9) * 4)",
        );
        expect(declarations.get("aspect-ratio")).toBe("1 / 1");
        expect(declarations.get("box-sizing")).toBe("border-box");
    });
});
