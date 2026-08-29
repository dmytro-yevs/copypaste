import { describe, expect, test } from "vitest";

import {
  mapWebViewPointToScreen,
  parseAppWindowFrame,
  parseDisplaySize,
} from "../src/harness/native-input.js";

describe("Android native input geometry", () => {
  const frame = { left: 0, top: 48, width: 1080, height: 1872 };
  const display = { width: 1080, height: 1920 };

  test("maps a CSS point through the physical app frame", () => {
    expect(
      mapWebViewPointToScreen(
        { x: 180, y: 320 },
        { width: 360, height: 624, devicePixelRatio: 3 },
        frame,
        display,
      ),
    ).toEqual({ x: 540, y: 1008 });
  });

  test.each([
    { x: -1, y: 10 },
    { x: 360, y: 10 },
    { x: 10, y: 624 },
  ])("rejects a point outside the CSS viewport: %o", (point) => {
    expect(() =>
      mapWebViewPointToScreen(
        point,
        { width: 360, height: 624, devicePixelRatio: 3 },
        frame,
        display,
      ),
    ).toThrow(/outside its viewport/);
  });

  test("rejects an app frame that extends outside the physical display", () => {
    expect(() =>
      mapWebViewPointToScreen(
        { x: 180, y: 320 },
        { width: 360, height: 624, devicePixelRatio: 3 },
        { left: 0, top: 48, width: 1080, height: 1900 },
        display,
      ),
    ).toThrow(/outside the display/);
  });

  test("prefers the effective override display size", () => {
    expect(
      parseDisplaySize("Physical size: 1440x2560\nOverride size: 1080x1920\n"),
    ).toEqual(display);
  });

  test("requires the app to own the foreground window and reads its frame", () => {
    const dump = [
      "mCurrentFocus=Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
      "  Window #4 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
      "    mFrame=[0,48][1080,1920]",
      "  Window #5 Window{def u0 com.android.systemui/.StatusBar}",
    ].join("\n");
    expect(parseAppWindowFrame(dump, "com.copypaste.app")).toEqual({
      left: 0,
      top: 48,
      width: 1080,
      height: 1872,
    });
    expect(() => parseAppWindowFrame(dump.replace("com.copypaste.app/", "com.android.settings/"), "com.copypaste.app")).toThrow(
      /foreground window/,
    );
  });
});
