/**
 * When the history caches are re-read, and how often the service is allowed to
 * be asked. The queries themselves live in `useHistory`; nothing here renders.
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

/**
 * For a write that re-sorts the list — pin, unpin, reorder, delete, clear. The
 * cached pages are then wrong about *where* rows are, not just about the head,
 * so they have to be re-walked.
 */
export async function invalidateHistoryQueries(qc: QueryClient): Promise<void> {
  await Promise.all([
    qc.invalidateQueries({ queryKey: HISTORY_KEY }),
    qc.invalidateQueries({ queryKey: HISTORY_SEARCH_KEY }),
  ]);
}

interface Coalesced {
  /** Ask for a run. Returns nothing: callers that must wait use `settled`. */
  readonly schedule: () => void;
  /** Resolves once the run that covers this call has finished refetching. */
  readonly settled: () => Promise<void>;
}

/**
 * A trailing edge with a ceiling, and never two runs at once.
 *
 * `lodash.debounce` owns the timer. The repository already ships it under
 * `usehooks-ts`, which is where `useHistoryController` gets its search debounce
 * from, and AGENTS.md rule 1 is the reason this does not become a sixth
 * hand-written scheduler. What the package cannot know is that one run here is
 * P serial IPC round trips and P page decrypts for P loaded pages: a second run
 * starting while the first is still walking amplifies instead of coalescing.
 * The in-flight guard defers, and re-enters the *same* debounce afterwards so
 * `maxWait` keeps bounding a stream that never pauses.
 */
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
    // Taken *before* the refetch, so a caller that registered while this run
    // was already reading waits for the next one. Releasing it here would hand
    // back a list read before its own write landed (INV-3).
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

const deepWalks = new WeakMap<QueryClient, Coalesced>();
const fullSweeps = new WeakMap<QueryClient, Coalesced>();

function coalescerFor(
  store: WeakMap<QueryClient, Coalesced>,
  qc: QueryClient,
  invalidate: (client: QueryClient) => Promise<unknown>,
): Coalesced {
  const existing = store.get(qc);
  if (existing !== undefined) return existing;
  const made = coalesce(qc, invalidate);
  store.set(qc, made);
  return made;
}

const invalidatePages = (qc: QueryClient) =>
  qc.invalidateQueries({ queryKey: HISTORY_PAGES_KEY });

/**
 * Manifest 06 §3.1.1 lists the events that must reach the window with no scroll
 * and no refocus, and a remote delete or pin three pages down is one of them —
 * nothing else reads the deeper pages, so they cannot be left stale.
 *
 * `HISTORY_KEY` is a *prefix* of the paged query's key, so invalidating it
 * walks every loaded page serially. The rate of clipboard changes is not
 * bounded, which is what the coalescer above is for.
 */
export async function invalidateHistoryHead(qc: QueryClient): Promise<void> {
  coalescerFor(deepWalks, qc, invalidatePages).schedule();
  // The head is immediate: it is where a capture lands, and waiting on the
  // coalescing window to show the user their own copy would be a regression.
  await Promise.all([
    qc.invalidateQueries({ queryKey: HISTORY_HEAD_KEY }),
    qc.invalidateQueries({ queryKey: HISTORY_SEARCH_KEY }),
  ]);
}

/**
 * For a write the caller has to wait on before it may show the rows again
 * (INV-3), where several such writes arrive in a row. A deferred delete commits
 * the one before it as soon as the next row is deleted (§3.1.8), so a user
 * clearing a handful of rows produces a run of commits a few hundred
 * milliseconds apart — one re-walk between them, not one each.
 */
export function coalesceHistoryInvalidation(qc: QueryClient): Promise<void> {
  return coalescerFor(fullSweeps, qc, invalidateHistoryQueries).settled();
}
