/**
 * React Query stands in for v1's hand-written polling, which is what manifest
 * §9.1 asks for; the contracts it has to keep are INV-2 (an idle poll must not
 * produce a new array identity), INV-3 (every mutation invalidates), INV-4 (a
 * poll must not replace the merged list with page 1), INV-27 (a hidden window
 * polls zero times) and INV-33 (last write wins). §5.5 says verify rather than
 * assume — `useHistory.test.tsx` does.
 *
 * The poll interval is now a function of whether the change stream is
 * delivering (`usePush`). It never becomes zero: see `pollInterval`.
 */
import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";

import { toFriendly } from "@/lib/errors";
import {
  PAGE_SIZE,
  POLL_ACTIVE_MS,
  POLL_BACKOFF_MS,
  POLL_PUSH_BACKSTOP_MS,
  SEARCH_LIMIT,
  STATUS_POLL_MS,
} from "@/lib/layout";
import {
  type Item,
  type ItemPage,
  type StatusData,
  copyItem,
  deleteAll,
  deleteItem,
  getStatus,
  listItems,
  reorderPinned,
  searchItems,
  setPinned,
} from "@/lib/ipc";

export const HISTORY_KEY = ["history"] as const;
export const STATUS_KEY = ["status"] as const;

export const historyKey = (query: string) => [...HISTORY_KEY, query] as const;

/** What `useHistory` hands the view: the merged rows and what was unreadable. */
export interface History {
  readonly items: readonly Item[];
  readonly skipped: number;
}

const EMPTY: History = { items: [], skipped: 0 };

/**
 * How often to re-read while push is live.
 *
 * Not `false`. A subscription can die without either end noticing — the daemon
 * is killed, the socket is half-closed by a sleep/wake — and a window that
 * stopped polling on the strength of one `live` event shows yesterday's
 * clipboard until it is restarted. Thirty seconds is a backstop, not a poll:
 * it is 2 requests a minute against the 20 that poll-only costs, and it bounds
 * how wrong the list can get to half a minute rather than to forever.
 */
export function pollInterval(pushLive: boolean, failed: boolean): number {
  if (failed) return POLL_BACKOFF_MS;
  return pushLive ? POLL_PUSH_BACKSTOP_MS : POLL_ACTIVE_MS;
}

/**
 * De-duplicated by id: a page offset is a position in a list a capture can
 * prepend to between two fetches, so one item can arrive on two pages (INV-4).
 * Module scope, because React Query memoises `select` on (data, selectFn) and
 * INV-2 needs the same object back when the data has not changed.
 *
 * The skipped counts sum across pages: three unreadable rows on page 1 and two
 * on page 2 is five items the user cannot see, and reporting only the last
 * page's count would quietly shrink as they scroll.
 */
function flatten(data: { pages: ItemPage[] }): History {
  const seen = new Set<string>();
  const items: Item[] = [];
  let skipped = 0;
  for (const page of data.pages) {
    skipped += page.skipped_undecryptable;
    for (const item of page.items) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      items.push(item);
    }
  }
  return { items, skipped };
}

/**
 * Search is deliberately **not** paged: `search` runs against the whole
 * database, so a match at index 800 is found without loading 800 rows first
 * (AT-73 / CopyPaste-crh3.106). Sensitive items are never indexed, so a search
 * legitimately cannot return one.
 */
export function useHistory(query: string, pushLive = false) {
  const searching = query.length > 0;

  return useInfiniteQuery({
    queryKey: historyKey(query),
    queryFn: ({ pageParam }) =>
      searching ? searchItems(query, SEARCH_LIMIT) : listItems(PAGE_SIZE, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      if (searching) return undefined;
      // Against `items.length`, not the page's total: a page that dropped
      // rows it could not decrypt is still a full page from the server, and
      // treating it as short would stop paging early and hide the rest of the
      // history behind a handful of corrupt rows.
      if (lastPage.items.length < PAGE_SIZE) return undefined;
      return allPages.reduce((total, page) => total + page.items.length, 0);
    },
    refetchInterval: (q) => pollInterval(pushLive, q.state.status === "error"),
    // Typing must not blank the list between keystrokes.
    placeholderData: keepPreviousData,
    select: flatten,
  });
}

/** The shape the view wants when the query has not resolved. */
export function historyOf(data: History | undefined): History {
  return data ?? EMPTY;
}

export function useStatus() {
  return useQuery<StatusData>({
    queryKey: STATUS_KEY,
    queryFn: getStatus,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : STATUS_POLL_MS,
  });
}

/* ----------------------------------------------------------- mutations --- */

/**
 * Not optimistic, and nothing here reorders locally: INV-31 (a pinned item
 * keeps its slot on copy while an unpinned one moves to the top) is then true
 * by construction rather than by a special case.
 *
 * The toast says what the user still has to do. Selecting an item makes it the
 * clipboard and nothing more — the app never synthesises ⌘V, because that
 * needs an Accessibility grant an ad-hoc-signed app loses on every update
 * (ADR-0001). Saying so at the moment of the copy is the difference between a
 * deliberate design and an app that looks broken.
 */
export function useCopy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => copyItem(item.id),
    onSuccess: () => {
      toast.success("Copied — press ⌘V to paste", { duration: 2500 });
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Refetches immediately, so the server's re-sort is what is shown. */
export function usePin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => setPinned(item.id, !item.pinned),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Destructive and un-undoable: the caller must gate it behind a confirm
 *  dialog (5j9x, kayk, fjvz, vcnv, w6xc are all one misclick away). */
export function useClearHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => deleteAll(),
    onSuccess: (removed) => {
      toast.success(
        `Cleared ${removed} item${removed === 1 ? "" : "s"} — pinned items kept`,
      );
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/* ---------------------------------------------------------------- bulk --- */

/** What a bulk run did. Partial failure is reported as partial (§3.1.9). */
export interface BulkOutcome {
  readonly done: number;
  readonly failed: number;
}

/**
 * Run one operation over a selection, reporting how much of it worked.
 *
 * Sequential, not `Promise.all`: every one of these is a write to one SQLite
 * connection behind one mutex, so concurrency buys nothing and a hundred
 * simultaneous requests is how a client trips the daemon's connection cap.
 * `allSettled` semantics without the parallelism.
 */
async function runBulk<T>(
  targets: readonly T[],
  each: (target: T) => Promise<unknown>,
): Promise<BulkOutcome> {
  let done = 0;
  let failed = 0;
  for (const target of targets) {
    try {
      await each(target);
      done += 1;
    } catch {
      // Swallowed deliberately: one failure must not abandon the rest of the
      // selection, and the raw error must not reach the view (INV-12).
      failed += 1;
    }
  }
  return { done, failed };
}

function report(verb: string, { done, failed }: BulkOutcome) {
  if (failed === 0) {
    toast.success(`${verb} ${done} item${done === 1 ? "" : "s"}`);
  } else {
    toast.warning(`${verb} ${done} of ${done + failed} — ${failed} failed`);
  }
}

/**
 * Pin or unpin a whole selection.
 *
 * `pinned` is decided by the caller from whether *every* selected item is
 * already pinned, so the button is one toggle with a label that matches what
 * it will do (CopyPaste-8ebg.55) rather than two buttons that are each wrong
 * half the time.
 */
export function useBulkPin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ items, pinned }: { items: readonly Item[]; pinned: boolean }) =>
      runBulk(items, (item) => setPinned(item.id, pinned)),
    onSuccess: (outcome, { pinned }) => {
      report(pinned ? "Pinned" : "Unpinned", outcome);
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Bulk delete has no undo (§3.1.9), so the caller must confirm first. */
export function useBulkDelete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (items: readonly Item[]) => runBulk(items, (item) => deleteItem(item.id)),
    onSuccess: (outcome) => {
      report("Deleted", outcome);
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/**
 * Rewrite the pinned order.
 *
 * Not optimistic. §3.1.6 specifies an optimistic reorder with a revert, and
 * that is right once the verb exists; while the bridge refuses it, an
 * optimistic move would show the user a new order, then take it away — which
 * reads as the app losing their change rather than as a missing feature.
 */
export function useReorderPinned() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: readonly string[]) => reorderPinned(ids),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
