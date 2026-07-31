/**
 * The diagnostics channel: what is running, what has been dropped, and the
 * block a user can paste into an issue.
 *
 * Its own module rather than a section of `lib/ipc.ts` because it has a rule
 * that file does not: **nothing here may be re-authored in TypeScript.**
 * `report` arrives already redacted by `copypaste_ipc::redact::scrub_paths` —
 * the one redactor — and building it, extending it or reformatting it on this
 * side would put a second one here (CLAUDE.md rule 1, and rule 4's no-paths
 * obligation). It is rendered verbatim or not at all.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, classifyError } from "@/lib/errors";
import { hasBridge, type ServiceState } from "@/lib/ipc";

/**
 * Cumulative since the service started, and never reset — so `uptime_secs` is
 * the only thing they can honestly be read against.
 *
 * Counts only. There is no field a clipping could travel in, which is what
 * makes "no clip content in diagnostics" structural rather than a habit.
 */
export interface DiagnosticCounters {
  /** Copies dropped for being over the size limit. */
  readonly rejected_too_large: number;
  /** Clipboard values overwritten before the service could read them. */
  readonly lost_intermediates: number;
  /** Detected secrets the auto-delete sweep removed. */
  readonly sensitive_swept: number;
  /** Search-index rows the startup purge removed. */
  readonly index_purged: number;
  readonly uptime_secs: number;
}

export interface DiagnosticsStatus {
  readonly version: string;
  readonly protocol_version: number;
  readonly item_count: number;
  readonly capture_running: boolean;
  readonly clipboard_backend: string;
  readonly legacy_history_present: boolean;
  readonly counters: DiagnosticCounters;
}

export interface Diagnostics {
  readonly app_version: string;
  readonly app_protocol_version: number;
  readonly os: string;
  readonly arch: string;
  readonly service: ServiceState;
  /** `null` when the service did not answer — itself the diagnosis. */
  readonly status: DiagnosticsStatus | null;
  /** Redacted in Rust. Rendered verbatim; never rebuilt here. */
  readonly report: string;
}

export function getDiagnostics(): Promise<Diagnostics> {
  if (!hasBridge()) throw new IpcFailure("offline");
  return invoke<Diagnostics>("diagnostics").catch((raw) => {
    throw new IpcFailure(classifyError(raw));
  });
}
