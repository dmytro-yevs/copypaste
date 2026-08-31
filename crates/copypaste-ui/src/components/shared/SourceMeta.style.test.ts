import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(
  resolve(process.cwd(), "src/components/shared/SourceMeta.module.css"),
  "utf8",
);

describe("SourceMeta app icon slot", () => {
  it("constrains the supplied app icon to the metadata glyph size", () => {
    expect(styles).toMatch(/\.app > \.sourceIcon\s*\{[^}]*inline-size:\s*var\(--fs-xs\)/);
    expect(styles).toMatch(/\.app > \.sourceIcon > \*\s*\{[^}]*inline-size:\s*100%/);
  });
});
