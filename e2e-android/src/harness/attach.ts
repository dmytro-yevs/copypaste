/**
 * When to keep waiting for the app's page target, when to re-resolve the
 * endpoint, and what the harness says when it gives up.
 *
 * Device-free, so a relaunch and a timeout can be driven without an emulator;
 * every other harness module reaches `adb.ts`, which shells out at import time.
 */

/** Where Tauri serves the frontend on Android; nothing else in the app has a page target. */
export const APP_ORIGIN = "http://tauri.localhost";

/** The whole origin, not the host and not a substring. `tauri.localhost.example.test`
 *  merely contains the name; `https://tauri.localhost/` and `http://tauri.localhost:444/`
 *  are the same name under a scheme and a port the app never serves, and each is a
 *  separate document a WebView could hold. Attaching to one drives it as the app.
 *  `URL.origin` normalises the default port, so `:80` is still the app. */
export function isAppTarget(url: string): boolean {
  try {
    return new URL(url).origin === APP_ORIGIN;
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

export interface AttachTargetObservation {
  type: string;
  appOrigin: boolean;
  webSocketPresent: boolean;
}

export interface AttachPagesDiagnostic {
  status: "ok" | "error";
  count: number;
  appOriginMatchCount: number;
}

export interface AttachRawDiagnostic {
  status: "ok" | "http-error" | "fetch-error" | "invalid-json";
  count: number;
  targetTypeHistogram: { page: number; webview: number; other: number };
  appOriginMatchCount: number;
  webSocketPresent: boolean;
}

export type DirectOutcome =
  | "not-attempted"
  | "connected"
  | "connect-failed"
  | "pages-failed"
  | "no-page"
  | "wrong-origin"
  | "ownership-failed"
  | "no-websocket"
  | "raw-empty"
  | "raw-error";

export interface AttachFinalDiagnostic {
  pages: AttachPagesDiagnostic;
  raw: AttachRawDiagnostic;
  directOutcome: DirectOutcome;
}

export function finalAttachDiagnostic(
  pages: AttachPagesDiagnostic,
  raw: AttachRawDiagnostic,
  directOutcome: DirectOutcome,
): AttachFinalDiagnostic {
  return { pages, raw, directOutcome };
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
    ? `pid ${sample.pid} exposes ${targets.length} page target(s)`
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
