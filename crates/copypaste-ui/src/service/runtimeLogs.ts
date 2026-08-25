import { call } from "@/lib/ipcCall";
import { UI_COMMANDS } from "@/generated/ipc";
import type {
  RuntimeLogEvent,
  RuntimeLogLevel,
  RuntimeLogPage,
  RuntimeLogProcess,
  RuntimeLogQuery as GeneratedRuntimeLogQuery,
} from "@/generated/ipc";

export type RuntimeLogQuery = Partial<GeneratedRuntimeLogQuery>;

export type {
  RuntimeLogEvent,
  RuntimeLogLevel,
  RuntimeLogPage,
  RuntimeLogProcess,
};

export function getRuntimeLogEvents(query: RuntimeLogQuery): Promise<RuntimeLogPage> {
  return call(UI_COMMANDS.runtime_log_events, { query });
}
