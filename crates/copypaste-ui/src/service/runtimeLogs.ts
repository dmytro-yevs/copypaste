import { call } from "@/lib/ipcCall";

export type RuntimeLogLevel = "error" | "warn" | "info" | "debug" | "trace";
export type RuntimeLogProcess = "app" | "daemon";

export interface RuntimeLogEvent {
  readonly timestamp_ms: number;
  readonly level: RuntimeLogLevel;
  readonly process: RuntimeLogProcess;
  readonly target: string;
  readonly message: string;
}

export interface RuntimeLogPage {
  readonly events: readonly RuntimeLogEvent[];
  readonly next_cursor: string | null;
}

export interface RuntimeLogQuery {
  readonly cursor?: string | null;
  readonly level?: RuntimeLogLevel | null;
  readonly process?: RuntimeLogProcess | null;
  readonly query?: string | null;
  readonly limit?: number;
}

export function getRuntimeLogEvents(query: RuntimeLogQuery): Promise<RuntimeLogPage> {
  return call<RuntimeLogPage>("runtime_log_events", { query });
}
