import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { runInNewContext } from "node:vm";

import { describe, expect, it, vi } from "vitest";

const document = readFileSync(resolve(process.cwd(), "index.html"), "utf8");

describe("Android system-bar insets bootstrap", () => {
  it("notifies native code before theme and application scripts run", () => {
    const ready = document.indexOf("window.__copypasteSystemBarInsets");
    const theme = document.indexOf("./theme-bootstrap.js");
    const app = document.indexOf('src="/src/main.tsx"');

    expect(ready).toBeGreaterThan(-1);
    expect(ready).toBeLessThan(theme);
    expect(ready).toBeLessThan(app);
  });

  it("calls the native bridge and tolerates its absence on desktop", () => {
    const script = document.match(/<script>\s*([\s\S]*?)<\/script>/)?.[1];
    expect(script).toBeDefined();
    const ready = vi.fn();
    runInNewContext(script!, { window: { __copypasteSystemBarInsets: { ready } } });
    expect(ready).toHaveBeenCalledOnce();
    expect(() => runInNewContext(script!, { window: {} })).not.toThrow();
    expect(script).not.toContain("?.");
  });
});
