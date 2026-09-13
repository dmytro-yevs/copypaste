import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

const globals = readFileSync(
  resolve(process.cwd(), "src/styles/globals.css"),
  "utf8",
);
const appFrame = readFileSync(
  resolve(process.cwd(), "src/components/layout/AppFrame.module.css"),
  "utf8",
);
const dock = readFileSync(
  resolve(process.cwd(), "src/app/shell/MobileDock.module.css"),
  "utf8",
);
const libraryScreen = readFileSync(
  resolve(process.cwd(), "src/features/history/screen/LibraryScreen.module.css"),
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

  it("keeps dock clearance when the IME owns the system inset", () => {
    expect(appFrame).toMatch(
      /html\[data-ime\][\s\S]*--scroll-dock-clearance:\s*calc\(var\(--s-9\) \+ var\(--s-2\) \+ var\(--s-3\)\)/,
    );
    expect(appFrame).not.toMatch(
      /html\[data-ime\][\s\S]*--scroll-dock-clearance:\s*calc\(var\(--s-1\) - var\(--s-1\)\)/,
    );
    expect(dock).toMatch(
      /html\[data-ime\]\)\s+\.dock\s*\{[^}]*inset-block-end:\s*var\(--s-3\);/s,
    );
    expect(libraryScreen).toMatch(
      /html\[data-ime\]\)\s+\.banners\s*\{[^}]*display:\s*none;/s,
    );
  });
});
