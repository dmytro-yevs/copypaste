/**
 * When to keep waiting for the app's page target, when to re-resolve the
 * endpoint, and what the harness says when it gives up.
 *
 * Device-free, so a relaunch and a timeout can be driven without an emulator;
 * every other harness module reaches `adb.ts`, which shells out at import time.
 */

/** Tauri serves the frontend from its own protocol; nothing else in the app has a page target. */
export const APP_ORIGIN = "tauri.localhost";

/** The host, not a substring: `tauri.localhost.example.test` contains the
 *  origin and is a different site, and a WebView that loaded one would have
 *  been attached to and driven as if it were the app. */
export function isAppTarget(url: string): boolean {
  try {
    return new URL(url).hostname === APP_ORIGIN;
  } catch {
    return false;
  }
}

export interface AttachSample {
  /** Page target urls, or undefined when the CDP connection stopped answering. */
  targets: string[] | undefined;
  /** The app's process now, or undefined when nothing runs under the package. */
  pid: number | undefined;
  /** The pid the forwarded devtools socket was resolved for. */
  endpointPid: number;
  msLeft: number;
}

export type AttachStep =
  | { do: "wait" }
  | { do: "reopen"; why: string }
  | { do: "give-up"; why: string };

/**
 * The socket name carries the app's pid (`devtools.ts`), so a process that
 * restarted is behind a forward that no longer resolves and a connection that
 * answers nothing. Re-resolving is the recovery; saying which of those happened
 * is what keeps a crash from reading as a slow start.
 */
function stale(sample: AttachSample): string | undefined {
  if (sample.pid === undefined) return `the app process (pid ${sample.endpointPid}) is gone`;
  if (sample.pid !== sample.endpointPid) {
    return `the app restarted (pid ${sample.endpointPid} → ${sample.pid})`;
  }
  if (sample.targets === undefined) {
    return `pid ${sample.pid} is running but its devtools connection stopped answering`;
  }
  return undefined;
}

function settled(sample: AttachSample): string {
  const targets = sample.targets ?? [];
  return targets.length
    ? `pid ${sample.pid} exposes ${targets.join(", ")}`
    : `pid ${sample.pid} is running and its WebView exposes no page target at all`;
}

export function nextAttachStep(sample: AttachSample): AttachStep {
  const why = stale(sample);
  if (sample.msLeft <= 0) return { do: "give-up", why: why ?? settled(sample) };
  return why === undefined ? { do: "wait" } : { do: "reopen", why };
}

/**
 * What the device said while the harness was failing to attach.
 *
 * Run 31671766432's API 29 legs were a bounded timeout with nothing to act on;
 * one `Tauri/Console` line in another leg's logcat — `Uncaught SyntaxError` out
 * of the WebView 74 that image carries — was the whole diagnosis. Reported
 * only: nothing here decides whether attachment succeeded, so a quiet crash
 * still fails.
 */
const COMPLAINT = /Tauri\/Console|AndroidRuntime|FATAL EXCEPTION|Fatal signal|Render process/;

export function webviewComplaints(logcat: string, keep = 5): string[] {
  const said = logcat
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => COMPLAINT.test(line));
  return [...new Set(said)].slice(-keep);
}
