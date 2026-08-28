import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

const globals = readFileSync(
  resolve(process.cwd(), "src/styles/globals.css"),
  "utf8",
);

describe("the native app frame", () => {
  it("locks document scrolling only for the Android WebView", () => {
    expect(globals).toMatch(
      /html\[data-platform="android"\]\s*\{[^}]*overflow:\s*hidden;[^}]*overscroll-behavior:\s*none;/s,
    );
    expect(globals).toMatch(
      /html\[data-platform="android"\]\s*#root\s*\{[^}]*position:\s*fixed;[^}]*inset:\s*0;[^}]*overflow:\s*hidden;/s,
    );
    expect(globals).not.toMatch(
      /html:(?:not|is)\([^)]*android[^)]*\)[^{]*\{[^}]*overflow:\s*hidden;/s,
    );
    expect(globals).not.toMatch(/(?:^|\n)\s*#root\s*\{[^}]*position:\s*fixed;/s);
  });
});
