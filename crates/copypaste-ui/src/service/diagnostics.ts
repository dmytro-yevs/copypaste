/**
 * Nothing here may be re-authored in TypeScript. `report` arrives redacted by
 * `copypaste_ipc::redact::scrub_paths` — the one redactor — and building,
 * extending or reformatting it on this side would put a second one here
 * (AGENTS.md rule 1, and rule 4's no-paths obligation).
 */
import { call } from "@/lib/ipcCall";
import { UI_COMMANDS } from "@/generated/ipc";
import type {
  Diagnostics,
  DiagnosticCounters,
  HistoryRead,
} from "@/generated/ipc";

export type { Diagnostics, DiagnosticCounters, HistoryRead };
export type DiagnosticsStatus = NonNullable<Diagnostics["status"]>;

const DIAGNOSTICS_TIMEOUT_MS = 4_000;

export function getDiagnostics(): Promise<Diagnostics> {
  return call(UI_COMMANDS.diagnostics, undefined, {
    timeoutMs: DIAGNOSTICS_TIMEOUT_MS,
  });
}

/** `false` means the system save panel was dismissed, never an error. */
export function exportDiagnosticsReport(): Promise<boolean> {
  return call(UI_COMMANDS.export_diagnostics_report);
}

/** Exports diagnostics plus bounded, redacted runtime events via native I/O. */
export function exportSupportBundle(): Promise<boolean> {
  return call(UI_COMMANDS.export_support_bundle);
}
