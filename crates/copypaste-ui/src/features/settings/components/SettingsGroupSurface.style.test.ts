import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const component = readFileSync(
  resolve(process.cwd(), "src/features/settings/components/SettingsGroupSurface.tsx"),
  "utf8",
);
const css = readFileSync(
  resolve(process.cwd(), "src/features/settings/components/SettingsGroupSurface.module.css"),
  "utf8",
);

describe("SettingsGroupSurface styles", () => {
  it("delegates surface chrome to the shared Surface primitive", () => {
    expect(component).toMatch(
      /<Surface\s+elevation="raised"\s+border="subtle"\s+radius="md"/,
    );
    expect(css).not.toMatch(
      /(?:border-color|border-radius|background|box-shadow|(?:-webkit-)?backdrop-filter)\s*:/,
    );
  });
});
