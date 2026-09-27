import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(
  resolve(process.cwd(), "src/features/quick-paste/screen/QuickPasteScreen.module.css"),
  "utf8",
);

describe("Quick Paste preview layout", () => {
  it("keeps the 403px list pane fixed around a transparent preview gap", () => {
    expect(styles).toMatch(/\.frame \{[^}]*display: flex;[^}]*gap: var\(--s-2\);[^}]*background: transparent;/);
    expect(styles).toMatch(/\.root \{[^}]*inline-size: 403px;[^}]*flex: none;/);
  });

  it("places the preview before the list only when native selects the left side", () => {
    expect(styles).toContain('.frame[data-preview-side="left"] .root { order: 2; }');
    expect(styles).toContain('.frame[data-preview-side="left"] > :last-child { order: 1; }');
  });
});
