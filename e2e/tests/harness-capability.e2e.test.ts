import { describe, expect, it } from "vitest";

import { assertTauriBrowserName } from "../src/harness/app.js";

describe("Tauri WebDriver capabilities", () => {
  it("accepts WebView2 only on Windows", () => {
    expect(() =>
      assertTauriBrowserName({ browserName: "webview2" }, "win32"),
    ).not.toThrow();
    expect(() => assertTauriBrowserName({ browserName: "wry" }, "win32")).toThrow(
      /expected the Tauri WebView \("webview2"\)/,
    );
  });

  it("retains the wry capability requirement off Windows", () => {
    expect(() =>
      assertTauriBrowserName({ browserName: "wry" }, "linux"),
    ).not.toThrow();
    expect(() => assertTauriBrowserName({ browserName: "webview2" }, "linux")).toThrow(
      /expected the Tauri WebView \("wry"\)/,
    );
  });

  it.each(["MicrosoftEdge", "chrome", undefined])(
    "rejects a browser-only session with browserName=%s",
    (browserName) => {
      expect(() => assertTauriBrowserName({ browserName }, "win32")).toThrow(
        /The session is not the app under test/,
      );
    },
  );
});
