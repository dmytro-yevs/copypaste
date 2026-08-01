import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const CSS = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");

describe("transient toast sizing", () => {
  it("keeps desktop feedback readable without a card-width default", () => {
    expect(CSS).toContain("min-width: min(280px, 100%)");
    expect(CSS).toContain("max-width: min(480px, 100%)");
    expect(CSS).toContain("width: min(480px");
  });

  it("uses the safe mobile width and leaves a text column beside close", () => {
    expect(CSS).toContain("width: 100% !important");
    expect(CSS).toContain("[data-content]");
    expect(CSS).toContain("flex: 1");
    expect(CSS).toContain("inset: -6px");
  });

  it("does not override Sonner's vertical expansion transform", () => {
    const toastRule = CSS.match(/\[data-sonner-toast\]\[data-styled="true"\] \{([\s\S]*?)\n\}/)?.[1] ?? "";
    expect(toastRule).not.toContain("transform:");
    expect(toastRule).not.toContain("position:");
  });

  it("keeps the delete Undo action compact and inline", () => {
    expect(CSS).toContain("[data-sonner-toast] [data-button]");
    expect(CSS).toContain("height: var(--ctl-h-sm)");
    expect(CSS).toContain("border-radius: var(--r-pill)");
  });

  it("centres desktop toasts in the content pane, not beneath the sidebar", () => {
    expect(CSS).toContain("var(--content-pane-center, 50%)");
    expect(CSS).toContain("data-x-position=\"center\"");
  });
});
