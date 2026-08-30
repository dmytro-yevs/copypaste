import { afterEach, describe, expect, test } from "vitest";

import {
  mapWebViewPointToScreen,
  parseAppWindowFrame,
  parseDisplaySize,
  foregroundFocusDiagnostic,
  tapNativeInput,
  withSoftKeyboardScenario,
  type NativeInputCommands,
} from "../src/harness/native-input.js";

const WINDOW_DUMP = [
  "mCurrentFocus=Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
  "  Window #4 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}",
  "    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,48][1080,1920] last=[1,49][1081,1921]",
  "  Window #5 Window{def u0 com.android.systemui/.StatusBar}",
].join("\n");
const WINDOW_SUBSECTION = WINDOW_DUMP.split("\n").slice(1).join("\n");
const WINDOW_BRIEF = WINDOW_DUMP.replace(
  "    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,48][1080,1920] last=[1,49][1081,1921]",
  "",
);
const LEGACY_WINDOW_DUMP = WINDOW_DUMP.replace(
  "Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,48][1080,1920] last=[1,49][1081,1921]",
  "mFrame=[0,48][1080,1920]",
);

const DISPLAY_OUTPUT = "Physical size: 1080x1920\n";
const POINT = { x: 180, y: 320 };
const METRICS = { width: 360, height: 624, devicePixelRatio: 3 };

function fixtureCommands(
  calls: string[][],
  serial = "device-a",
  serialAnswer = serial,
  failShell = false,
  windowDump = WINDOW_DUMP,
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
      return args[0] === "wm" ? DISPLAY_OUTPUT : windowDump;
    },
    tryShell: async (selected, ...args) => {
      calls.push(["-s", selected, ...args]);
      return { ok: true, value: "" };
    },
  };
}

function softKeyboardCommands(
  calls: string[][],
  initial: "0" | "1" | null,
  options: {
    readValues?: string[];
    failRestore?: boolean;
    failTap?: boolean;
    imeWindow?: boolean;
  } = {},
): NativeInputCommands {
  let value = initial;
  let settingsWrites = 0;
  const reads = [...(options.readValues ?? [])];
  return {
    devices: async () => {
      calls.push(["devices"]);
      return "List of devices attached\ndevice-a\tdevice\n";
    },
    getSerialno: async (serial) => {
      calls.push(["-s", serial, "get-serialno"]);
      return serial;
    },
    shell: async (serial, ...args) => {
      calls.push(["-s", serial, ...args]);
      if (args[0] !== "settings") {
        if (args[0] === "wm") return DISPLAY_OUTPUT;
        return WINDOW_DUMP;
      }
      if (args[1] === "get") return reads.shift() ?? value ?? "null";
      settingsWrites += 1;
      if (options.failRestore && settingsWrites === 2) {
        throw new Error("restore failed");
      }
      value = args[1] === "delete" ? null : args[4]! as "0" | "1";
      return "";
    },
    tryShell: async (serial, ...args) => {
      calls.push(["-s", serial, ...args]);
      if (args[0] === "settings") return { ok: true, value: value ?? "null" };
      if (args[0] === "dumpsys") {
        return { ok: true, value: options.imeWindow ? "  mInputMethodWindow=Window{safe}" : "" };
      }
      if (options.failTap) {
        return { ok: false, failure: { message: "tap failed" } };
      }
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
    ).toThrow(/foreground focus/);
  });

  test("reads the modern frame field, not parent, display, or last", () => {
    expect(parseAppWindowFrame(WINDOW_DUMP, "com.copypaste.app")).toEqual({
      left: 0,
      top: 48,
      width: 1080,
      height: 1872,
    });
  });

  test("retains the legacy mFrame field for older Android dumps", () => {
    expect(parseAppWindowFrame(LEGACY_WINDOW_DUMP, "com.copypaste.app")).toEqual({
      left: 0,
      top: 48,
      width: 1080,
      height: 1872,
    });
  });

  test("rejects a brief full dump that has focus but no WindowFrames", () => {
    expect(() => parseAppWindowFrame(WINDOW_BRIEF, "com.copypaste.app")).toThrow(
      /window frame/,
    );
  });

  test("selects the exact focused window among same-package windows", () => {
    const dump = WINDOW_DUMP.replace(
      "  Window #4 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}\n    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,48][1080,1920] last=[1,49][1081,1921]",
      "  Window #4 Window{def u0 com.copypaste.app/com.copypaste.app.MainActivity}\n    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,0][1080,100] last=[0,0][1080,100]\n  Window #5 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}\n    Frames: parent=[0,0][1080,1920] display=[0,0][1080,1920] frame=[0,48][1080,1920] last=[1,49][1081,1921]",
    );
    expect(parseAppWindowFrame(dump, "com.copypaste.app")).toEqual({
      left: 0,
      top: 48,
      width: 1080,
      height: 1872,
    });
  });

  test.each([
    ["current16full", WINDOW_DUMP, "present", "com.copypaste.app", "com.copypaste.app.MainActivity"],
    ["current16subsection", WINDOW_SUBSECTION, "missing", null, null],
    ["missing", "Window #4 Window{abc u0 com.copypaste.app/com.copypaste.app.MainActivity}", "missing", null, null],
    ["null", "mCurrentFocus=null", "null", null, null],
    ["other package", WINDOW_DUMP.replace("com.copypaste.app/", "com.android.settings/"), "present", "com.android.settings", "com.copypaste.app.MainActivity"],
  ] as const)("reports sanitized %s focus diagnostics", (_name, dump, status, packageName, component) => {
    expect(foregroundFocusDiagnostic(dump)).toEqual({ status, packageName, component });
  });

  test("passes the selected serial to every native command after discovery", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await tapNativeInput(POINT, METRICS, "com.copypaste.app", fixtureCommands(calls));
    expect(calls).toEqual([
      ["devices"],
      ["-s", "device-a", "get-serialno"],
      ["-s", "device-a", "wm", "size"],
      ["-s", "device-a", "dumpsys", "window", "-a"],
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

  test("does not tap when Android 16's window subsection omits focus", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "device-a", "device-a", false, WINDOW_SUBSECTION),
      ),
    ).rejects.toThrow(/"status":"missing"/);
    expect(calls.some((call) => call.includes("input"))).toBe(false);
  });

  test("does not tap when the focused window has no usable frame", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    const malformed = WINDOW_DUMP.replace(/frame=\[[^\n]+/, "frame=broken");
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "device-a", "device-a", false, malformed),
      ),
    ).rejects.toThrow(/window frame/);
    expect(calls.some((call) => call.includes("input"))).toBe(false);
  });

  test("does not tap when the brief dump omits WindowFrames", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      tapNativeInput(
        POINT,
        METRICS,
        "com.copypaste.app",
        fixtureCommands(calls, "device-a", "device-a", false, WINDOW_BRIEF),
      ),
    ).rejects.toThrow(/window frame/);
    expect(calls.some((call) => call.includes("input"))).toBe(false);
  });

  test("pins the serial, verifies soft-keyboard setup, and restores zero after a tap", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await withSoftKeyboardScenario(
      (scenario) => scenario.tap(POINT, METRICS, "com.copypaste.app"),
      softKeyboardCommands(calls, "0"),
    );
    expect(calls).toEqual([
      ["devices"],
      ["-s", "device-a", "get-serialno"],
      ["-s", "device-a", "settings", "get", "secure", "show_ime_with_hard_keyboard"],
      ["-s", "device-a", "settings", "put", "secure", "show_ime_with_hard_keyboard", "1"],
      ["-s", "device-a", "settings", "get", "secure", "show_ime_with_hard_keyboard"],
      ["-s", "device-a", "wm", "size"],
      ["-s", "device-a", "dumpsys", "window", "-a"],
      ["-s", "device-a", "input", "tap", "540", "1008"],
      ["-s", "device-a", "settings", "put", "secure", "show_ime_with_hard_keyboard", "0"],
      ["-s", "device-a", "settings", "get", "secure", "show_ime_with_hard_keyboard"],
    ]);
  });

  test.each(["1", null] as const)("restores soft-keyboard preference %j exactly", async (original) => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await withSoftKeyboardScenario(async () => undefined, softKeyboardCommands(calls, original));
    const restore = original === null
      ? ["settings", "delete", "secure", "show_ime_with_hard_keyboard"]
      : ["settings", "put", "secure", "show_ime_with_hard_keyboard", original];
    expect(calls.at(-2)).toEqual(["-s", "device-a", ...restore]);
  });

  test("restores the soft-keyboard preference when the native tap fails", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      withSoftKeyboardScenario(
        (scenario) => scenario.tap(POINT, METRICS, "com.copypaste.app"),
        softKeyboardCommands(calls, "0", { failTap: true }),
      ),
    ).rejects.toThrow(/native Android tap failed/);
    expect(calls.at(-2)).toEqual([
      "-s", "device-a", "settings", "put", "secure", "show_ime_with_hard_keyboard", "0",
    ]);
  });

  test("does not tap when soft-keyboard readback is not enabled", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      withSoftKeyboardScenario(
        async () => undefined,
        softKeyboardCommands(calls, "0", { readValues: ["0", "2", "0"] }),
      ),
    ).rejects.toThrow(/was unavailable/);
    expect(calls.some((call) => call.includes("input"))).toBe(false);
    expect(calls.at(-2)).toEqual([
      "-s", "device-a", "settings", "put", "secure", "show_ime_with_hard_keyboard", "0",
    ]);
  });

  test("returns a restoration failure after a successful callback", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      withSoftKeyboardScenario(async () => undefined, softKeyboardCommands(calls, "0", { failRestore: true })),
    ).rejects.toThrow("restore failed");
  });

  test("preserves callback failure when restoration also fails", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await expect(
      withSoftKeyboardScenario(
        async () => {
          throw new Error("scenario failed");
        },
        softKeyboardCommands(calls, "0", { failRestore: true }),
      ),
    ).rejects.toThrow("scenario failed");
  });

  test("returns only redacted serial-bound IME diagnostics", async () => {
    delete process.env.ANDROID_SERIAL;
    const calls: string[][] = [];
    await withSoftKeyboardScenario(
      async (scenario) => {
        expect(await scenario.diagnostics()).toEqual({ preference: "1", imeWindow: "present" });
      },
      softKeyboardCommands(calls, "0", { imeWindow: true }),
    );
    expect(calls.filter((call) => call.includes("dumpsys"))).toEqual([
      ["-s", "device-a", "dumpsys", "window", "-a"],
    ]);
  });
});
