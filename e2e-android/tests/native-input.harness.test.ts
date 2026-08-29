import { afterEach, describe, expect, test } from "vitest";

import {
  mapWebViewPointToScreen,
  parseAppWindowFrame,
  parseDisplaySize,
  tapNativeInput,
  type NativeInputCommands,
} from "../src/harness/native-input.js";

const WINDOW_DUMP = [
  "mCurrentFocus=Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
  "  Window #4 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
  "    mFrame=[0,48][1080,1920]",
  "  Window #5 Window{def u0 com.android.systemui/.StatusBar}",
].join("\n");

const DISPLAY_OUTPUT = "Physical size: 1080x1920\n";
const POINT = { x: 180, y: 320 };
const METRICS = { width: 360, height: 624, devicePixelRatio: 3 };

function fixtureCommands(
  calls: string[][],
  serial = "device-a",
  serialAnswer = serial,
  failShell = false,
): NativeInputCommands {
  return {
    devices: async () => {
      calls.push(["devices"]);
      return `List of devices attached\n${serial}\tdevice\n`;
    },
    getSerialno: async (selected) => {
      calls.push(["-s", selected, "get-serialno"]);
      return serialAnswer;
    },
    shell: async (selected, ...args) => {
      calls.push(["-s", selected, ...args]);
      if (failShell) throw new Error("device command failed");
      return args[0] === "wm" ? DISPLAY_OUTPUT : WINDOW_DUMP;
    },
    tryShell: async (selected, ...args) => {
      calls.push(["-s", selected, ...args]);
      return { ok: true, value: "" };
    },
  };
}

const configuredSerial = process.env.ANDROID_SERIAL;
afterEach(() => {
  if (configuredSerial === undefined) delete process.env.ANDROID_SERIAL;
  else process.env.ANDROID_SERIAL = configuredSerial;
});

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
    expect(parseAppWindowFrame(WINDOW_DUMP, "com.copypaste.app")).toEqual({
      left: 0,
      top: 48,
      width: 1080,
      height: 1872,
    });
    expect(() =>
      parseAppWindowFrame(
        WINDOW_DUMP.replace("com.copypaste.app/", "com.android.settings/"),
        "com.copypaste.app",
      ),
    ).toThrow(/foreground window/);
  });

  test("passes the selected serial to every native command after discovery", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await tapNativeInput(POINT, METRICS, "com.copypaste.app", fixtureCommands(calls));
    expect(calls).toEqual([
      ["devices"],
      ["-s", "device-a", "get-serialno"],
      ["-s", "device-a", "wm", "size"],
      ["-s", "device-a", "dumpsys", "window", "windows"],
      ["-s", "device-a", "input", "tap", "540", "1008"],
    ]);
  });

  test("keeps a configured serial bound and rejects a replacement device", async () => {
    process.env.ANDROID_SERIAL = "configured-device";
    const calls: string[][] = [];
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "configured-device", "replacement-device"),
      ),
    ).rejects.toThrow(/expected configured-device/);
    expect(calls).toEqual([
      ["-s", "configured-device", "get-serialno"],
    ]);
  });

  test("rejects a no-env replacement instead of tapping the sole new device", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "device-a", "replacement-device"),
      ),
    ).rejects.toThrow(/expected device-a/);
    expect(calls).toEqual([
      ["devices"],
      ["-s", "device-a", "get-serialno"],
    ]);
  });

  test("does not tap when a serial-bound preflight command fails", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "device-a", "device-a", true),
      ),
    ).rejects.toThrow("device command failed");
    expect(calls.some((call) => call.includes("input"))).toBe(false);
  });
});
