/**
 * Nothing here may be re-authored in TypeScript. `report` arrives redacted by
 * `copypaste_ipc::redact::scrub_paths` — the one redactor — and building,
 * extending or reformatting it on this side would put a second one here
 * (CLAUDE.md rule 1, and rule 4's no-paths obligation).
 */
import { call } from "@/lib/ipcCall";
import type { ServiceState } from "@/lib/ipc";

/** Cumulative since the service started and never reset, so `uptime_secs` is
 *  the only thing they can honestly be read against. Counts only: there is no
 *  field a clipping could travel in. */
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
  readonly counters: DiagnosticCounters;
}

/** A bounded one-item read of the same history query the History screen uses. */
export type HistoryRead =
  | { readonly state: "readable" }
  | { readonly state: "failed"; readonly code: string };

export interface Diagnostics {
  readonly app_version: string;
  readonly app_protocol_version: number;
  readonly os: string;
  readonly arch: string;
  readonly service: ServiceState;
  /** `null` when the service did not answer — itself the diagnosis. */
  readonly status: DiagnosticsStatus | null;
  /** A stable error code only; no daemon message or clipboard content. */
  readonly history_read: HistoryRead;
  /** Redacted in Rust. Rendered verbatim; never rebuilt here. */
  readonly report: string;
}

export function getDiagnostics(): Promise<Diagnostics> {
  return call<Diagnostics>("diagnostics");
}

/** `false` means the system save panel was dismissed, never an error. */
export function exportDiagnosticsReport(): Promise<boolean> {
  return call<boolean>("export_diagnostics_report");
}

/** Exports diagnostics plus bounded, redacted runtime events via native I/O. */
export function exportSupportBundle(): Promise<boolean> {
  return call<boolean>("export_support_bundle");
}
