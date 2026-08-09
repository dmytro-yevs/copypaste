import { describe, expect, it } from "vitest";

import {
  assertTauriBridge,
  assertTauriBrowserName,
} from "../src/harness/app.js";

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

describe("Tauri bridge startup", () => {
  it("waits only for WebView2's initial about:blank", () => {
    expect(() =>
      assertTauriBridge({ bridge: false, url: "about:blank" }, false),
    ).not.toThrow();
    expect(() =>
      assertTauriBridge({ bridge: false, url: "about:blank" }, true),
    ).toThrow(/no IPC is under test/);
  });

  it("rejects another bridgeless page immediately", () => {
    expect(() =>
      assertTauriBridge({ bridge: false, url: "http://localhost:1420/" }, false),
    ).toThrow(/no IPC is under test/);
  });
});
