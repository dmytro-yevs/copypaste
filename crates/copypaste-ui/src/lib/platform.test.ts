import { describe, expect, it } from "vitest";

import { isAndroid } from "@/lib/platform";

describe("isAndroid", () => {
  it("recognises Android WebView without hiding desktop controls in browsers", () => {
    expect(isAndroid("Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36")).toBe(true);
    expect(isAndroid("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe(false);
  });
});
