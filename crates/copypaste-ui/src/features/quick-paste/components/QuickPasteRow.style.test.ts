import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const component = readFileSync(
  resolve(process.cwd(), "src/features/quick-paste/components/QuickPasteRow.tsx"),
  "utf8",
);
const css = readFileSync(
  resolve(process.cwd(), "src/features/quick-paste/components/QuickPasteRow.module.css"),
  "utf8",
);

describe("QuickPasteRow compact presentation", () => {
  it("uses a flat row with a fixed one-line quick-paste density", () => {
    expect(component).toMatch(/elevation="flat"\s+border="none"\s+radius="sm"/);
    expect(component).toContain("previewLines: QUICK_PASTE_PREVIEW_LINES");
    expect(component).toContain('surface: "quickPaste"');
  });

  it("keeps desktop shortcuts and a keyboard-reachable quiet pin control", () => {
    expect(css).toMatch(/\.shortcut \{[\s\S]*?display: inline-flex;/);
    expect(css).not.toContain("@media (--cp-lg)");
    expect(css).toMatch(/\.pinAction:focus-visible \{ opacity: 1; \}/);
    expect(css).toMatch(/\.root\[data-pinned="true"\] \.pinAction/);
  });

  it("uses compact fine-pointer rows and restores touch targets on coarse pointers", () => {
    expect(css).toMatch(/\.root \{[\s\S]*?min-block-size: var\(--ctl-h-sm\);/);
    expect(css).toMatch(/@media \(pointer: coarse\) \{[\s\S]*?min-block-size: var\(--tap-min\);/);
  });

  it("aligns the source icon with the first content line", () => {
    expect(css).toMatch(/\.root \{[\s\S]*?align-items: flex-start;/);
    expect(css).toMatch(/\.sourceIcon \{[\s\S]*?margin-block-start: var\(--quick-paste-icon-offset\);/);
    const sizes = JSON.parse(readFileSync(resolve(process.cwd(), "../../design/tokens/size.json"), "utf8")).size;
    expect(sizes["quick-paste-icon-offset"].$value).toBe("2.5px");
  });

  it("reserves the action column for every clip kind, including images", () => {
    expect(css).toMatch(/\.body \{[\s\S]*?padding-inline-end: var\(--quick-paste-actions\);/);
    expect(css).not.toContain('.root[data-kind="image"] .body { padding-inline-end: 0; }');
    expect(css).toMatch(/\.root\[data-kind="image"\] \.body > \* \{[\s\S]*?max-inline-size: min\(100%, calc\(var\(--s-9\) \* 6\)\);/);
  });

  it("does not move rows on hover", () => {
    const hoverRule = css.match(/\.root:hover \{(?<rule>[\s\S]*?)\}/)?.groups?.rule;
    expect(hoverRule).toBeDefined();
    expect(hoverRule).not.toContain("transform");
  });

  it("does not render a competing row tooltip", () => {
    expect(component).not.toContain("TooltipRoot");
    expect(component).not.toContain("TooltipContent");
  });
});
