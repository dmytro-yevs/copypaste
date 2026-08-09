import { execa } from "execa";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const metadataTool = fileURLToPath(new URL("../../../scripts/release/android-metadata.mjs", import.meta.url));
const APP_NAMESPACE = execFileSync(
  process.execPath,
  [metadataTool, "--field", "releaseApplicationId"],
  { encoding: "utf8" },
).trim();
export const PACKAGE = process.env.COPYPASTE_PACKAGE ?? APP_NAMESPACE;
export const MAIN_ACTIVITY = `${PACKAGE}/${APP_NAMESPACE}.MainActivity`;
export const INTAKE_ACTIVITY = `${PACKAGE}/${APP_NAMESPACE}.IntakeActivity`;

/**
 * `adb` from the SDK the emulator came from, not whatever is on PATH: a second
 * copy starts a second server, which steals the device from the first one and
 * reports it as offline.
 */
function binary(): string {
  if (process.env.ADB) return process.env.ADB;
  const root =
    process.env.ANDROID_SDK_ROOT ??
    process.env.ANDROID_HOME ??
    (process.env.LOCALAPPDATA ? `${process.env.LOCALAPPDATA}/Android/Sdk` : undefined);
  if (!root) return "adb";
  return `${root}/platform-tools/adb${process.platform === "win32" ? ".exe" : ""}`;
}

function target(): string[] {
  const serial = process.env.ANDROID_SERIAL;
  return serial ? ["-s", serial] : [];
}

export async function adb(...args: string[]): Promise<string> {
  const { stdout } = await execa(binary(), [...target(), ...args], { timeout: 60_000 });
  // adb reports the device's LF output with CRLF on Windows.
  return stdout.replace(/\r\n/g, "\n");
}

/**
 * adb concatenates the argument vector into one string and the device's shell
 * parses it again, so anything with a space has to survive that second parse.
 */
export function quote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

export async function shell(...args: string[]): Promise<string> {
  return adb("shell", ...args);
}

/**
 * The app's own process, not a WebView renderer.
 *
 * Same rule as `app_pid` in `scripts/release/android-smoke-lib.sh`: Chromium's
 * sandboxed children are named after the package too and `pidof` prints every
 * match in an order nothing documents, so its first token is a coin toss over
 * which process the next read describes. An exact name match is not.
 */
export async function appPid(): Promise<number | undefined> {
  const exact = (await shell("ps", "-A", "-o", "PID,NAME").catch(() => ""))
    .split("\n")
    .map((line) => line.trim().split(/\s+/))
    .find(([, name]) => name === PACKAGE);
  const pid = Number(exact?.[0] ?? (await shell("pidof", PACKAGE).catch(() => "")).trim().split(/\s+/)[0]);
  return Number.isInteger(pid) && pid > 0 ? pid : undefined;
}

export async function isDebuggable(): Promise<boolean> {
  const dump = await shell("dumpsys", "package", PACKAGE);
  return /flags=\[[^\]]*\bDEBUGGABLE\b/.test(dump);
}

export async function launchMain(): Promise<void> {
  await shell("am", "start", "-W", "-n", MAIN_ACTIVITY);
}

/**
 * The share-sheet doorway, which is the only way to put a known clipping into
 * history without a clipboard the emulator will not hand us
 * (docs/rewrite/android-spike.md: rung 2 needs a Shizuku pairing).
 */
export async function shareText(text: string): Promise<void> {
  await shell(
    "am",
    "start",
    "-a",
    "android.intent.action.SEND",
    "-t",
    "text/plain",
    "--es",
    "android.intent.extra.TEXT",
    quote(text),
    "-n",
    INTAKE_ACTIVITY,
  );
}

export async function forward(port: number, socket: string): Promise<void> {
  await adb("forward", "--remove", `tcp:${port}`).catch(() => undefined);
  await adb("forward", `tcp:${port}`, `localabstract:${socket}`);
}

export async function removeForward(port: number): Promise<void> {
  await adb("forward", "--remove", `tcp:${port}`).catch(() => undefined);
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
