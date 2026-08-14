/**
 * What a failed adb invocation means, and its text kept intact.
 *
 * Device-free, like `attach.ts` and `readiness.ts`, so the classification can be
 * driven without an emulator.
 *
 * `appPid()` read `adb shell ps` through `.catch(() => "")`, so a transport that
 * was not answering produced an empty process list and the harness concluded
 * "no process named com.copypaste.app is running" — the app named for a failure
 * that was the transport's, retried to a 90s deadline and then reported as a
 * slow app. Nothing here decides that a command succeeded; it only says what a
 * failure was, and every classification carries adb's own words forward.
 */

import type { Probe } from "./readiness.js";

export interface CommandFailure {
  exitCode?: number;
  stderr?: string;
  stdout?: string;
  message?: string;
}

/**
 * adb prefixes a transport error with its own argv[0] for the shell service and
 * with `error:` for others — observed on 1.0.41 / platform-tools 36.0.0 as
 * `adb.exe: device 'emulator-9999' not found` from `adb -s … shell ps` and
 * `error: device 'emulator-9999' not found` from `adb -s … get-state`. Matching
 * the prefix would therefore miss half the cases; these match the sentence.
 */
const GONE = [
  /\bdevice '[^']*' not found\b/,
  /\bno devices\/emulators found\b/,
  /\bdevice (?:unauthorized|still authorizing)\b/,
];

/**
 * `unauthorized` and `still authorizing` are from adb's documented output and
 * are not reproducible on a machine with no device; they are grouped with the
 * two that were observed because none of the three becomes a running app by
 * being asked again, and the harness has already asserted exactly one attached
 * device in `global-setup.ts` before any of this runs.
 */
export function adbFailureText(failure: CommandFailure): string {
  const said = [failure.stderr, failure.stdout]
    .map((stream) => (stream ?? "").trim())
    .filter(Boolean)
    .join(" ");
  const text = said || (failure.message ?? "").trim() || "adb failed and said nothing";
  return failure.exitCode === undefined ? text : `${text} (adb exit ${failure.exitCode})`;
}

/**
 * Transient by default: an unrecognised failure is carried to the caller's final
 * message verbatim rather than being read as a device state. A wait that ends on
 * one reports adb's sentence, which is the whole point of the type.
 */
export function classifyAdbFailure(failure: CommandFailure): Probe<never> {
  const why = adbFailureText(failure);
  return isDeviceGone(why) ? { kind: "invariant", why } : { kind: "transient", why };
}

export function isDeviceGone(text: string): boolean {
  return GONE.some((pattern) => pattern.test(text));
}

/**
 * `adb shell pidof <package>` exits 1 with nothing on either stream when no
 * process matches, which is an answer and not a failure. Reading it as one is
 * the mirror of the defect above: it would turn "the app is not running" into
 * "adb is broken" and fail a wait that was working correctly.
 */
export function isEmptyDeviceAnswer(failure: CommandFailure): boolean {
  const said = `${failure.stderr ?? ""}${failure.stdout ?? ""}`.trim();
  return said === "" && failure.exitCode !== undefined && failure.exitCode !== 0;
}
