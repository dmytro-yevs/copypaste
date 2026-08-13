import { useQuery } from "@tanstack/react-query";

import { POLL_BACKOFF_MS, STATUS_POLL_MS } from "@/lib/layout";
import { type StatusData, getStatus } from "@/lib/ipc";
import { STATUS_KEY } from "@/hooks/historyRefresh";

export { STATUS_KEY };

/**
 * `select` is not an optimisation of the poll — the 2s cadence is a defect fix
 * (`CopyPaste-f701`) and stays. It is what stops `counters.uptime_secs`, which
 * ticks every second, from handing every consumer a new object twice a second
 * and re-rendering the whole history subtree at idle. Pass a *module-scope*
 * selector naming only the fields the caller renders; structural sharing then
 * returns the previous result. `error` and `isPending` are unaffected, so a
 * dead daemon is still detected within one poll.
 */
export function useStatus<T = StatusData>(select?: (data: StatusData) => T) {
  return useQuery<StatusData, Error, T>({
    queryKey: STATUS_KEY,
    queryFn: getStatus,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : STATUS_POLL_MS,
    select,
  });
}

export const statusReachable = (): true => true;
export const statusItemCount = (data: StatusData): number => data.item_count;
export const statusDeviceName = (data: StatusData): string => data.device_name;

/** Module scope, not an inline arrow: React Query memoises on the selector's
 *  identity, and a fresh closure each render defeats the structural sharing
 *  that makes these return the previous object. */
export const statusOwnDevice = (data: StatusData) => ({
  device_name: data.device_name,
  capture_running: data.capture_running,
  private_mode: data.private_mode,
  version: data.version,
  item_count: data.item_count,
});

export const statusService = (data: StatusData) => ({
  version: data.version,
  capture_running: data.capture_running,
  clipboard_backend: data.clipboard_backend,
  protocol_version: data.protocol_version,
  item_count: data.item_count,
});
