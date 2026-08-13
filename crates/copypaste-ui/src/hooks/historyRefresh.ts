/**
 * When the history caches are re-read, and how often. The queries themselves
 * live in `useHistory`; nothing here renders.
 */
import type { QueryClient } from "@tanstack/react-query";
import debounce from "lodash.debounce";

import { HISTORY_COALESCE_MAX_MS, HISTORY_COALESCE_MS } from "@/lib/layout";

export const HISTORY_KEY = ["history"] as const;
export const HISTORY_SEARCH_KEY = ["history-search"] as const;
export const STATUS_KEY = ["status"] as const;

export const HISTORY_HEAD_KEY = [...HISTORY_KEY, "head"] as const;
export const HISTORY_PAGES_KEY = [...HISTORY_KEY, "pages"] as const;

export const historyKey = (query: string) =>
  [...HISTORY_PAGES_KEY, query] as const;

interface Coalesced {
  readonly schedule: ReturnType<typeof debounce>;
  readonly settled: () => Promise<void>;
}

// lodash.debounce + in-flight guard: one walk is P serial IPC round trips
// and P page decrypts, so a second walk while the first is reading amplifies
// instead of coalescing. The guard defers, and re-enters the same debounce
// so `maxWait` keeps bounding a stream that never pauses.
function coalesce(
  qc: QueryClient,
  invalidate: (client: QueryClient) => Promise<unknown>,
): Coalesced {
  let running = false;
  let again = false;
  let waiting: (() => void)[] = [];

  const run = () => {
    if (running) {
      again = true;
      return;
    }
    running = true;
    const covered = waiting;
    waiting = [];
    void invalidate(qc).finally(() => {
      running = false;
      if (again) {
        again = false;
        schedule();
      }
      for (const resolve of covered) resolve();
    });
  };

  const schedule = debounce(run, HISTORY_COALESCE_MS, {
    leading: false,
    trailing: true,
    maxWait: HISTORY_COALESCE_MAX_MS,
  });

  return {
    schedule,
    settled: () =>
      new Promise<void>((resolve) => {
        waiting.push(resolve);
        schedule();
      }),
  };
}

/** One shared store: peak concurrent page walks is exactly one, across
 *  capture events, deferred-delete refreshes, and user-initiated writes. */
const walks = new WeakMap<QueryClient, Coalesced>();

function walksFor(qc: QueryClient): Coalesced {
  const existing = walks.get(qc);
  if (existing !== undefined) return existing;
  const made = coalesce(qc, (client) =>
    client.invalidateQueries({ queryKey: HISTORY_PAGES_KEY }),
  );
  walks.set(qc, made);
  return made;
}

/** Manifest 06 §3.1.1: a capture or push must reach the window without
 *  scroll or refocus. Head and search are immediate; the page walk is
 *  coalesced so a burst doesn't amplify. */
export async function invalidateHistoryHead(qc: QueryClient): Promise<void> {
  walksFor(qc).schedule();
  await Promise.all([
    qc.invalidateQueries({ queryKey: HISTORY_HEAD_KEY }),
    qc.invalidateQueries({ queryKey: HISTORY_SEARCH_KEY }),
  ]);
}

/** For a write the caller must wait on before showing the rows again (INV-3),
 *  where several such writes arrive in a row. */
export function coalesceHistoryInvalidation(qc: QueryClient): Promise<void> {
  void qc.invalidateQueries({ queryKey: HISTORY_HEAD_KEY });
  void qc.invalidateQueries({ queryKey: HISTORY_SEARCH_KEY });
  return walksFor(qc).settled();
}

/** For a write that re-sorts the list — pin, unpin, reorder, delete, clear.
 *  Bypasses the debounce via flush, but still serialises with the in-flight
 *  guard so peak walks stays one. */
export async function invalidateHistoryQueries(qc: QueryClient): Promise<void> {
  const c = walksFor(qc);
  const done = c.settled();
  c.schedule.flush();
  await Promise.all([
    done,
    qc.invalidateQueries({ queryKey: HISTORY_HEAD_KEY }),
    qc.invalidateQueries({ queryKey: HISTORY_SEARCH_KEY }),
  ]);
}
