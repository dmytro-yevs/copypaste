import {
  adb,
  adbForSerial,
  PACKAGE,
  shellForSerial,
  tryShellForSerial,
  type Attempt,
} from "./adb.js";
import { adbFailureText } from "./adb-failure.js";

export interface DisplaySize {
  width: number;
  height: number;
}

export interface WindowFrame extends DisplaySize {
  left: number;
  top: number;
}

export interface WebViewMetrics extends DisplaySize {
  devicePixelRatio: number;
}

export interface WebViewPoint {
  x: number;
  y: number;
}

export interface ScreenPoint {
  x: number;
  y: number;
}

export interface NativeTapReceipt {
  serial: string;
  point: ScreenPoint;
  frame: WindowFrame;
  display: DisplaySize;
}

type ShowImeWithHardKeyboard = "0" | "1" | null;

export interface SoftKeyboardDiagnostics {
  preference: ShowImeWithHardKeyboard | "unknown";
  imeWindow: "present" | "unknown";
}

export interface SoftKeyboardScenario {
  serial: string;
  diagnostics: () => Promise<SoftKeyboardDiagnostics>;
  tap: (
    point: WebViewPoint,
    metrics: WebViewMetrics,
    packageName?: string,
  ) => Promise<NativeTapReceipt>;
}

export type FocusDiagnostic = {
  status: "present" | "missing" | "null" | "malformed";
  packageName: string | null;
  component: string | null;
};

type FocusIdentity = FocusDiagnostic & { token: string | null };

export interface NativeInputCommands {
  devices: () => Promise<string>;
  getSerialno: (serial: string) => Promise<string>;
  shell: (serial: string, ...args: string[]) => Promise<string>;
  tryShell: (serial: string, ...args: string[]) => Promise<Attempt<string>>;
}

const commands: NativeInputCommands = {
  devices: () => adb("devices"),
  getSerialno: (serial) => adbForSerial(serial, "get-serialno"),
  shell: (serial, ...args) => shellForSerial(serial, ...args),
  tryShell: (serial, ...args) => tryShellForSerial(serial, ...args),
};

function positiveInteger(value: string): number | undefined {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : undefined;
}

export function parseDisplaySize(output: string): DisplaySize {
  const override = output.match(/(?:^|\n)\s*Override size:\s*(\d+)x(\d+)/);
  const physical = output.match(/(?:^|\n)\s*Physical size:\s*(\d+)x(\d+)/);
  const match = override ?? physical;
  const width = match && positiveInteger(match[1]!);
  const height = match && positiveInteger(match[2]!);
  if (!width || !height) throw new Error("Android display size was unavailable");
  return { width, height };
}

function parseFrame(value: string): WindowFrame | undefined {
  const match = value.match(
    /(?:mFrame=|\bframe=)(?:Rect\()?\[?(-?\d+),\s*(-?\d+)\]?\s*\[?(-?\d+),\s*(-?\d+)\]?\)?/,
  );
  if (!match) return undefined;
  const left = Number(match[1]);
  const top = Number(match[2]);
  const right = Number(match[3]);
  const bottom = Number(match[4]);
  if (![left, top, right, bottom].every(Number.isInteger)) return undefined;
  if (right <= left || bottom <= top) return undefined;
  return { left, top, width: right - left, height: bottom - top };
}

export function foregroundFocusDiagnostic(dump: string): FocusDiagnostic {
  const { status, packageName, component } = focusIdentity(dump);
  return { status, packageName, component };
}

function focusIdentity(dump: string): FocusIdentity {
  const line = dump.split("\n").find((candidate) => /\bmCurrentFocus\s*=/.test(candidate));
  if (!line) return { status: "missing", packageName: null, component: null, token: null };
  if (/\bmCurrentFocus\s*=\s*(?:null|Window\{[^}]*\snull\s*\}?)/.test(line)) {
    return { status: "null", packageName: null, component: null, token: null };
  }
  const match = line.match(
    /\bmCurrentFocus\s*=\s*Window\{([^\s]+)\s+u\d+\s+([^\s/]+)\/([^\s}]+)/,
  );
  if (!match) {
    return { status: "malformed", packageName: null, component: null, token: null };
  }
  return {
    status: "present",
    packageName: match[2]!,
    component: match[3]!,
    token: match[1]!,
  };
}

export function parseAppWindowFrame(
  dump: string,
  packageName = PACKAGE,
): WindowFrame {
  const focus = focusIdentity(dump);
  if (focus.status !== "present") {
    throw new Error(
      `Android foreground focus ${JSON.stringify({
        status: focus.status,
        packageName: focus.packageName,
        component: focus.component,
      })}; expected ${packageName}`,
    );
  }
  if (focus.packageName !== packageName) {
    throw new Error(
      `Android foreground focus ${JSON.stringify({
        status: focus.status,
        packageName: focus.packageName,
        component: focus.component,
      })} does not match expected ${packageName}`,
    );
  }

  // AOSP WindowManagerService emits WindowFrames only when dumpAll (`-a`) is
  // set; the native probe requests that flag for focus and frame atomically.
  const lines = dump.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    const header = lines[index]!.match(
      /^\s*Window #\d+ Window\{([^\s]+)\s+u\d+\s+([^\s/]+)\/([^\s}]+)\}/,
    );
    if (
      !header ||
      header[1] !== focus.token ||
      header[2] !== packageName ||
      header[2] !== focus.packageName ||
      header[3] !== focus.component
    ) {
      continue;
    }
    for (let cursor = index; cursor < lines.length; cursor += 1) {
      if (cursor > index && /Window #\d+ Window\{/.test(lines[cursor]!)) break;
      const frame = parseFrame(lines[cursor]!);
      if (frame) return frame;
    }
  }
  throw new Error(`Android window frame for ${packageName} was unavailable`);
}

export function mapWebViewPointToScreen(
  point: WebViewPoint,
  metrics: WebViewMetrics,
  frame: WindowFrame,
  display: DisplaySize,
): ScreenPoint {
  if (
    ![
      point.x,
      point.y,
      metrics.width,
      metrics.height,
      metrics.devicePixelRatio,
    ].every(Number.isFinite) ||
    metrics.width <= 0 ||
    metrics.height <= 0 ||
    metrics.devicePixelRatio <= 0 ||
    point.x < 0 ||
    point.x >= metrics.width ||
    point.y < 0 ||
    point.y >= metrics.height
  ) {
    throw new Error("WebView tap point was outside its viewport");
  }
  if (
    frame.left < 0 ||
    frame.top < 0 ||
    frame.width <= 0 ||
    frame.height <= 0 ||
    frame.left + frame.width > display.width ||
    frame.top + frame.height > display.height
  ) {
    throw new Error("Android app window frame was outside the display");
  }

  // The app frame is physical pixels; CSS boxes are in the WebView viewport.
  const scaleX = frame.width / metrics.width;
  const scaleY = frame.height / metrics.height;
  if (
    Math.abs(scaleX - metrics.devicePixelRatio) > 0.25 ||
    Math.abs(scaleY - metrics.devicePixelRatio) > 0.25
  ) {
    throw new Error("Android WebView scale did not match its window frame");
  }
  const screen = {
    x: Math.round(frame.left + point.x * scaleX),
    y: Math.round(frame.top + point.y * scaleY),
  };
  if (
    screen.x < frame.left ||
    screen.x >= frame.left + frame.width ||
    screen.y < frame.top ||
    screen.y >= frame.top + frame.height ||
    screen.x >= display.width ||
    screen.y >= display.height
  ) {
    throw new Error("mapped Android tap point was outside the display");
  }
  return screen;
}

async function selectedSerial(nativeCommands: NativeInputCommands): Promise<string> {
  const selected = process.env.ANDROID_SERIAL?.trim();
  if (selected) {
    const actual = (await nativeCommands.getSerialno(selected)).trim();
    if (actual !== selected) {
      throw new Error(`adb selected serial ${actual || "unknown"}, expected ${selected}`);
    }
    return selected;
  }
  const attached = (await nativeCommands.devices())
    .split("\n")
    .slice(1)
    .filter((line) => /\tdevice$/.test(line))
    .map((line) => line.split("\t", 1)[0]!)
    .filter(Boolean);
  if (attached.length !== 1) {
    throw new Error(`native tap needs one attached Android device, found ${attached.length}`);
  }
  const serial = attached[0]!;
  const actual = (await nativeCommands.getSerialno(serial)).trim();
  if (actual !== serial) {
    throw new Error(`adb selected serial ${actual || "unknown"}, expected ${serial}`);
  }
  return serial;
}

function showImeWithHardKeyboard(output: string): ShowImeWithHardKeyboard {
  const value = output.trim();
  if (value === "0" || value === "1") return value;
  if (value === "null") return null;
  throw new Error("Android show_ime_with_hard_keyboard was unavailable");
}

async function readShowImeWithHardKeyboard(
  serial: string,
  nativeCommands: NativeInputCommands,
): Promise<ShowImeWithHardKeyboard> {
  return showImeWithHardKeyboard(
    await nativeCommands.shell(
      serial,
      "settings",
      "get",
      "secure",
      "show_ime_with_hard_keyboard",
    ),
  );
}

async function restoreShowImeWithHardKeyboard(
  serial: string,
  original: ShowImeWithHardKeyboard,
  nativeCommands: NativeInputCommands,
): Promise<void> {
  if (original === null) {
    await nativeCommands.shell(
      serial,
      "settings",
      "delete",
      "secure",
      "show_ime_with_hard_keyboard",
    );
  } else {
    await nativeCommands.shell(
      serial,
      "settings",
      "put",
      "secure",
      "show_ime_with_hard_keyboard",
      original,
    );
  }
  if (await readShowImeWithHardKeyboard(serial, nativeCommands) !== original) {
    throw new Error("Android show_ime_with_hard_keyboard was not restored");
  }
}

function imeWindowDiagnostic(output: string): "present" | "unknown" {
  // Android 14 and 16 emit this full-dump line only for a current IME window.
  // Nothing infers absence because dump variants may omit it.
  return /^\s*mInputMethodWindow=Window\{[^\n]*\}$/m.test(output)
    ? "present"
    : "unknown";
}

async function softKeyboardDiagnostics(
  serial: string,
  nativeCommands: NativeInputCommands,
): Promise<SoftKeyboardDiagnostics> {
  const [preference, window] = await Promise.all([
    nativeCommands.tryShell(
      serial,
      "settings",
      "get",
      "secure",
      "show_ime_with_hard_keyboard",
    ),
    nativeCommands.tryShell(serial, "dumpsys", "window", "-a"),
  ]);
  return {
    preference: preference.ok ? (() => {
      try {
        return showImeWithHardKeyboard(preference.value);
      } catch {
        return "unknown" as const;
      }
    })() : "unknown",
    imeWindow: window.ok ? imeWindowDiagnostic(window.value) : "unknown",
  };
}

async function tapNativeInputForSerial(
  serial: string,
  point: WebViewPoint,
  metrics: WebViewMetrics,
  packageName: string,
  nativeCommands: NativeInputCommands,
): Promise<NativeTapReceipt> {
  const [displayOutput, windowOutput] = await Promise.all([
    nativeCommands.shell(serial, "wm", "size"),
    nativeCommands.shell(serial, "dumpsys", "window", "-a"),
  ]);
  const display = parseDisplaySize(displayOutput);
  const frame = parseAppWindowFrame(windowOutput, packageName);
  const screenPoint = mapWebViewPointToScreen(point, metrics, frame, display);
  const tap = await nativeCommands.tryShell(
    serial,
    "input",
    "tap",
    String(screenPoint.x),
    String(screenPoint.y),
  );
  if (!tap.ok) {
    throw new Error(
      `native Android tap failed: ${adbFailureText(tap.failure)}`,
    );
  }
  return { serial, point: screenPoint, frame, display };
}

export async function withSoftKeyboardScenario<T>(
  callback: (scenario: SoftKeyboardScenario) => Promise<T>,
  nativeCommands: NativeInputCommands = commands,
): Promise<T> {
  const serial = await selectedSerial(nativeCommands);
  const original = await readShowImeWithHardKeyboard(serial, nativeCommands);
  let primaryFailure: unknown;
  try {
    await nativeCommands.shell(
      serial,
      "settings",
      "put",
      "secure",
      "show_ime_with_hard_keyboard",
      "1",
    );
    if (await readShowImeWithHardKeyboard(serial, nativeCommands) !== "1") {
      throw new Error("Android show_ime_with_hard_keyboard did not enable");
    }
    return await callback({
      serial,
      diagnostics: () => softKeyboardDiagnostics(serial, nativeCommands),
      tap: (point, metrics, packageName = PACKAGE) =>
        tapNativeInputForSerial(serial, point, metrics, packageName, nativeCommands),
    });
  } catch (error) {
    primaryFailure = error;
    throw error;
  } finally {
    try {
      await restoreShowImeWithHardKeyboard(serial, original, nativeCommands);
    } catch (error) {
      if (primaryFailure === undefined) throw error;
      console.warn(
        `Android show_ime_with_hard_keyboard cleanup also failed: ${String(error)}`,
      );
    }
  }
}

export async function tapNativeInput(
  point: WebViewPoint,
  metrics: WebViewMetrics,
  packageName = PACKAGE,
  nativeCommands: NativeInputCommands = commands,
): Promise<NativeTapReceipt> {
  const serial = await selectedSerial(nativeCommands);
  return tapNativeInputForSerial(serial, point, metrics, packageName, nativeCommands);
}
