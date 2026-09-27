import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

const document = readFileSync(resolve(process.cwd(), "index.html"), "utf8");

describe("Android system-bar insets bootstrap", () => {
  it("notifies native code before theme and application scripts run", () => {
    const ready = document.indexOf("window.__copypasteSystemBarInsets?.ready?.()");
    const theme = document.indexOf("./theme-bootstrap.js");
    const app = document.indexOf('src="/src/main.tsx"');

    expect(ready).toBeGreaterThan(-1);
    expect(ready).toBeLessThan(theme);
    expect(ready).toBeLessThan(app);
  });
});
