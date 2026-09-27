import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(
  resolve(process.cwd(), "src/features/quick-paste/screen/QuickPasteScreen.module.css"),
  "utf8",
);
const previewStyles = readFileSync(
  resolve(process.cwd(), "src/features/quick-paste/components/QuickPastePreview.module.css"),
  "utf8",
);

describe("Quick Paste preview layout", () => {
  it("keeps the 403px list pane fixed while the preview owns its transparent gap", () => {
    expect(styles).toMatch(/\.frame \{[^}]*display: flex;[^}]*background: transparent;/);
    expect(styles.match(/\.frame \{[^}]*gap:/)).toBeNull();
    expect(styles).toMatch(/\.root \{[^}]*inline-size: 403px;[^}]*flex: none;/);
  });

  it("places the preview before the list only when native selects the left side", () => {
    expect(styles).toContain('.frame[data-preview-side="left"] .root { order: 2; }');
    expect(styles).toContain('.frame[data-preview-side="left"] > :last-child { order: 1; }');
  });

  it("keeps the native 403px plus 320px allocation inside the outer window", () => {
    const listWidth = 403;
    const nativePreviewWidth = 320;
    const gap = 8;

    expect(listWidth + nativePreviewWidth).toBe(723);
    expect(nativePreviewWidth - gap).toBe(312);
    expect(previewStyles).toContain('.pane[data-side="right"] { padding-inline-start: var(--s-2); }');
    expect(previewStyles).toContain('.pane[data-side="left"] { padding-inline-end: var(--s-2); }');
  });
});
