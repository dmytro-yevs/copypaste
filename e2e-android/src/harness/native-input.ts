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

export type FocusDiagnostic = {
  status: "present" | "missing" | "null" | "malformed";
  packageName: string | null;
  component: string | null;
};

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
    /mFrame=(?:Rect\()?\[?(-?\d+),\s*(-?\d+)\]?\s*\[?(-?\d+),\s*(-?\d+)\]?\)?/,
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
  const line = dump.split("\n").find((candidate) => /\bmCurrentFocus\s*=/.test(candidate));
  if (!line) return { status: "missing", packageName: null, component: null };
  if (/\bmCurrentFocus\s*=\s*(?:null|Window\{[^}]*\snull\s*\}?)/.test(line)) {
    return { status: "null", packageName: null, component: null };
  }
  const match = line.match(/\bmCurrentFocus\s*=\s*Window\{[^\n]*\s([^\s/]+)\/([^\s}]+)/);
  if (!match) return { status: "malformed", packageName: null, component: null };
  return { status: "present", packageName: match[1]!, component: match[2]! };
}

export function parseAppWindowFrame(
  dump: string,
  packageName = PACKAGE,
): WindowFrame {
  const focus = foregroundFocusDiagnostic(dump);
  if (focus.status !== "present") {
    throw new Error(
      `Android foreground focus ${JSON.stringify(focus)}; expected ${packageName}`,
    );
  }
  if (focus.packageName !== packageName) {
    throw new Error(
      `Android foreground focus ${JSON.stringify(focus)} does not match expected ${packageName}`,
    );
  }

  // AOSP WindowManagerService's `windows` subcommand dumps only the window
  // list; the full dump carries mCurrentFocus and the same window frames.
  const lines = dump.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    if (!/Window #\d+ Window\{/.test(lines[index]!) || !lines[index]!.includes(packageName)) {
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

export async function tapNativeInput(
  point: WebViewPoint,
  metrics: WebViewMetrics,
  packageName = PACKAGE,
  nativeCommands: NativeInputCommands = commands,
): Promise<NativeTapReceipt> {
  const serial = await selectedSerial(nativeCommands);
  const [displayOutput, windowOutput] = await Promise.all([
    nativeCommands.shell(serial, "wm", "size"),
    nativeCommands.shell(serial, "dumpsys", "window"),
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
