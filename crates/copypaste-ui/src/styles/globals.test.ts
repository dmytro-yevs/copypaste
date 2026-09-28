import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");
const main = readFileSync(resolve(process.cwd(), "src/main.tsx"), "utf8");

describe("Quick Paste window surface", () => {
  it("leaves the WebView corners transparent while retaining the rounded panel", () => {
    expect(main).toContain('document.documentElement.dataset.surface = protectedSurface ? desktopSurface : isQuickPaste ? "quick-paste" : "main";');
    expect(globals).toMatch(/html\[data-surface="quick-paste"\] \{\s*background: transparent;/);
  });
});
