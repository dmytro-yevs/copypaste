import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const screenCss = readFileSync(
  resolve(process.cwd(), "src/features/settings/screen/SettingsScreen.module.css"),
  "utf8",
);
const rowCss = readFileSync(
  resolve(process.cwd(), "src/components/shared/SettingsRow.module.css"),
  "utf8",
);
const aboutCss = readFileSync(
  resolve(process.cwd(), "src/features/settings/patterns/AboutTab.module.css"),
  "utf8",
);

describe("Settings search highlight", () => {
  it("uses one long highlight animation for rows and sections", () => {
    expect(screenCss).toMatch(
      /\[data-settings-search-highlight="true"\][\s\S]*animation:\s*settings-search-highlight var\(--settings-search-highlight-duration\)/,
    );
    expect(screenCss).toMatch(/14%, 72%/);
    expect(rowCss).not.toContain("search-highlight");
  });

  it("keeps resource links free of browser-default underlines", () => {
    const openRow = aboutCss.match(/\.openRow\s*\{([^}]*)\}/);
    expect(openRow?.[1]).toMatch(/text-decoration:\s*none/);
  });
});
