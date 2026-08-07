import { describe, expect, it } from "vitest";

import { isAndroid, isWindows } from "@/lib/platform";

const ANDROID = "Mozilla/5.0 (Linux; Android 14; Pixel) AppleWebKit/537.36";
const MACOS = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)";
const WEBVIEW2 =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";

describe("isAndroid", () => {
  it("recognises Android WebView without hiding desktop controls in browsers", () => {
    expect(isAndroid(ANDROID)).toBe(true);
    expect(isAndroid(MACOS)).toBe(false);
  });
});

describe("isWindows", () => {
  it("recognises WebView2 and nothing else", () => {
    expect(isWindows(WEBVIEW2)).toBe(true);
    expect(isWindows(MACOS)).toBe(false);
  });

  /** An Android user agent can contain "Windows" through a spoofed or unusual
   *  build string; the Android check has to win or a phone is told to hold
   *  Ctrl. */
  it("never claims a phone is a desktop", () => {
    expect(isWindows("Mozilla/5.0 (Linux; Android 14; Windows Phone lookalike)")).toBe(false);
  });
});
