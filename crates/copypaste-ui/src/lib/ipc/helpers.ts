// ---------------------------------------------------------------------------
// lib/ipc/helpers.ts — formatters, status utilities, and IPC error helpers.
// Zero Tauri runtime dependency; safe to import in tests without mocks.
// ---------------------------------------------------------------------------

import { api } from "./api";
import { IpcError } from "./transport";
import { appVersion } from "./tauriCommands";
import type { DaemonStatus, StatusProbe } from "./types";

/**
 * Derive a short readable label from a macOS bundle id.
 * "com.google.Chrome" → "Chrome", "com.apple.Safari" → "Safari".
 * Falls back to the raw bundle id when it doesn't contain a dot, or to
 * an empty string when the id is absent.
 */
export function sourceAppLabel(bundleId: string | null | undefined): string {
  if (!bundleId) return "";
  const parts = bundleId.split(".");
  const last = parts[parts.length - 1];
  // Title-case the last segment (handles e.g. "terminal" → "Terminal").
  return last.charAt(0).toUpperCase() + last.slice(1);
}

/** Format Unix epoch milliseconds for display. */
export function formatWallTime(ms: number): string {
  if (ms <= 0) return "—";
  return new Date(ms).toLocaleString();
}

/**
 * Format a Unix timestamp in seconds (as stored in `PairedDevice.added_at`)
 * for human-readable display. Returns "—" for falsy/zero values.
 */
export function formatEpochSecs(secs: number | null | undefined): string {
  if (!secs) return "—";
  return new Date(secs * 1000).toLocaleString();
}

/**
 * Returns true when a daemon content_type represents an image — either the
 * bare legacy token "image" or any MIME-typed "image/*" variant.
 * Single source of truth shared by HistoryView, Popup, and notification logic.
 */
export function isImageType(ct: string): boolean {
  return ct === "image" || ct.startsWith("image/");
}

/**
 * Extract a human-readable message from a caught error, falling back to
 * `fallback` when the error is not an IpcError (e.g. a transport TypeError).
 * Use at every catch site instead of the repeated ternary.
 *
 * @param err      The value caught in a catch clause (type `unknown`).
 * @param fallback Fallback string when `err` is not an IpcError.
 */
export function ipcErrorMessage(err: unknown, fallback: string): string {
  return err instanceof IpcError ? err.message : fallback;
}

/**
 * Map a caught error to a safe, user-facing string that NEVER leaks socket
 * paths, usernames, or internal Rust error strings into the DOM.
 *
 * Rules:
 *  - Known IPC error codes → canonical friendly copy.
 *  - Unknown IpcError      → "Something went wrong." (code is logged, not rendered).
 *  - Non-IpcError          → "Something went wrong." (never serialises raw transport
 *                            strings that may contain socket paths or usernames).
 *
 * Use this wherever `err.message` or `String(err)` would otherwise be rendered
 * as visible text. Console-logging the raw error for diagnostics is fine —
 * just never put it in the DOM.
 */
export function friendlyIpcError(err: unknown): string {
  if (!(err instanceof IpcError)) {
    // Non-IpcError (e.g. TypeError, transport string) — never leak internals.
    return "Something went wrong.";
  }
  switch (err.code) {
    case "daemon_offline":
      return "The background service is not running.";
    case "ipc_not_ready":
      return "The background service is starting up. Please wait a moment.";
    case "not_found":
    case "NotFound":
      return "The requested item was not found.";
    case "permission_denied":
    case "PermissionDenied":
      return "Permission denied.";
    case "migration_in_progress":
      return "A database migration is in progress. Please wait.";
    case "version_mismatch":
      return "The app and background service are on incompatible versions. Please restart.";
    case "rate_limited":
      return "Too many requests. Please wait and try again.";
    default:
      // Unknown code — return generic copy. Do NOT include err.message: it may
      // contain socket paths or other internal strings.
      return "Something went wrong.";
  }
}

/**
 * Returns true when the error represents the daemon being alive but not yet
 * ready to serve requests (e.g. still initialising its database). Daemon
 * error code: `"ipc_not_ready"`. Views should show a friendly "starting up"
 * state rather than a hard error when this returns true.
 *
 * CopyPaste-crh3.9: the legacy uppercase `"IPC_NOT_READY"` compat branch was
 * removed — the wire `error_code` (transport.ts) is always lowercase, so it was
 * unreachable dead code.
 */
export function isIpcNotReady(err: unknown): boolean {
  if (!(err instanceof IpcError)) return false;
  return err.code === "ipc_not_ready";
}

/**
 * Probe the daemon's status and collapse it to a {@link StatusProbe}. Never
 * throws — a transport failure resolves to `{ kind: "offline" }` so every caller
 * has a defined, non-blank failure path.
 */
export async function probeStatus(): Promise<StatusProbe> {
  try {
    const s = (await api.status()) as Partial<DaemonStatus>;
    if (s && (s.degraded === true || s.ready === false)) {
      return { kind: "degraded", reason: s.degraded_reason ?? null };
    }
    return { kind: "ok" };
  } catch {
    // Transport-level failure (daemon offline) — IpcError or otherwise.
    return { kind: "offline" };
  }
}

/**
 * Parse a semver string into [major, minor, patch] numbers.
 * Returns null if the string cannot be parsed as semver.
 */
function parseSemver(ver: string): [number, number, number] | null {
  const parts = ver.split(".");
  if (parts.length < 3) return null;
  const nums = parts.slice(0, 3).map(Number);
  if (nums.some(isNaN)) return null;
  return nums as [number, number, number];
}

/**
 * Return -1 if a < b, 0 if equal, 1 if a > b (semver comparison).
 */
function compareSemver(
  a: [number, number, number],
  b: [number, number, number]
): -1 | 0 | 1 {
  for (let i = 0; i < 3; i++) {
    if (a[i] < b[i]) return -1;
    if (a[i] > b[i]) return 1;
  }
  return 0;
}

/**
 * Inspect a pre-fetched {@link DaemonStatus} and app version string to decide
 * if the daemon is stale (running an OLDER build than the app). Returns the
 * daemon's reported version string when stale, `"unknown"` when it predates
 * the `build_version` field, or `null` when not stale (same version, daemon
 * is NEWER, or comparison isn't possible).
 *
 * Only flags as stale when the daemon is strictly OLDER — a daemon that is
 * NEWER than the app (e.g. the user rolled back) is not flagged so the banner
 * doesn't appear in that direction.
 */
export function detectStaleDaemonFromStatus(
  status: Partial<DaemonStatus> | null,
  appVer: string
): string | null {
  if (!status) return null;
  const reported = status.build_version ?? null;
  // No version field => daemon predates this build => stale by definition.
  if (reported === null || reported === "") return "unknown";
  const reportedPrefix = reported.split("+")[0];
  if (reportedPrefix === appVer) return null;
  // Parse both as semver to determine direction.
  const daemonParsed = parseSemver(reportedPrefix);
  const appParsed = parseSemver(appVer);
  if (!daemonParsed || !appParsed) {
    // Cannot parse — fall back to string inequality: flag as stale when different.
    return reportedPrefix !== appVer ? reported : null;
  }
  const cmp = compareSemver(daemonParsed, appParsed);
  // Only stale when daemon is strictly OLDER (cmp === -1).
  return cmp === -1 ? reported : null;
}

/**
 * Compare the running daemon's build to the app's own. Returns the daemon's
 * version when it is STALE (survived an upgrade — strictly OLDER semver),
 * else `null`.
 *
 * Only flags as stale when the daemon is strictly OLDER than the app. A daemon
 * that is NEWER (e.g. after a rollback) is NOT flagged. Best-effort: any error
 * (e.g. daemon offline) yields `null` so callers never block startup on this check.
 */
export async function detectStaleDaemon(): Promise<string | null> {
  let appVer: string;
  let status: DaemonStatus;
  try {
    [appVer, status] = await Promise.all([appVersion(), api.status()]);
  } catch {
    return null;
  }
  return detectStaleDaemonFromStatus(status, appVer);
}
