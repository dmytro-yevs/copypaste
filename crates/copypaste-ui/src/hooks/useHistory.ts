/**
 * The contracts React Query has to keep here: INV-2 (an idle poll must not
 * produce a new array identity), INV-3 (every mutation invalidates), INV-4 (a
 * poll must not replace the merged list with page 1), INV-27 (a hidden window
 * polls zero times) and INV-33 (last write wins). `useHistory.test.tsx`
 * verifies rather than assumes (§5.5).
 */
import {
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
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
 * Never `false`. A subscription can die without either end noticing — the
 * daemon is killed, the socket is half-closed by a sleep/wake — and a window
 * that stopped polling on the strength of one `live` event shows yesterday's
 * clipboard until it is restarted. The backstop bounds how wrong the list can
 * get to half a minute rather than to forever, at 2 requests a minute against
 * poll-only's 20.
 */
export function pollInterval(pushLive: boolean, failed: boolean): number {
  if (failed) return POLL_BACKOFF_MS;
  return pushLive ? POLL_PUSH_BACKSTOP_MS : POLL_ACTIVE_MS;
}

/**
 * De-duplicated by id: a cursor cannot hand the same row out twice, but a pin
 * moves a row between sections mid-walk, so two held pages can still name it
 * (INV-4). First occurrence wins, so the list does not jump under the scroll.
 *
 * Module scope: React Query memoises `select` on (data, selectFn), and INV-2
 * needs the same object back when the data has not changed.
 *
 * Skipped counts sum across pages; the last page's alone would shrink as the
 * user scrolls.
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
 * Paged by cursor, never by offset (B-1, `CopyPaste-8ebg.57`). A capture
 * landing above the window shifts every offset below it, so page 2 repeats a
 * row or skips one — and a clipping the user never saw is indistinguishable
 * from one that was never captured (rule 4). A cursor names a position in the
 * order, so anything inserted above it is not in the page after it.
 *
 * The page param is the previous page's `next_cursor`, opaque here. Search is
 * not paged at all.
 */
export function useHistory(query: string, pushLive = false) {
  const searching = query.length > 0;

  return useInfiniteQuery({
    queryKey: historyKey(query),
    queryFn: ({ pageParam }) =>
      searching ? searchItems(query, SEARCH_LIMIT) : listItems(PAGE_SIZE, pageParam),
    initialPageParam: null as string | null,
    // `next_cursor`, and nothing derived from the page's length: rows counted
    // in `skipped_undecryptable` were read and dropped, so a short page is not
    // the end of the list, and stopping on one would hide the rest of the
    // history behind a handful of corrupt rows.
    getNextPageParam: (lastPage) => (searching ? undefined : lastPage.next_cursor ?? undefined),
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
 * The toast says what the user still has to do, because the app never
 * synthesises ⌘V (ADR-0001).
 */
export function useCopy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => copyItem(item.id),
    onSuccess: () => {
      toast.success(t("history.toast.copied"), { duration: 2500 });
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
      toast.success(t("history.toast.cleared", { count: removed }));
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
 * Sequential, not `Promise.all`: every one of these is a write to one SQLite
 * connection behind one mutex, so concurrency buys nothing and a hundred
 * simultaneous requests is how a client trips the daemon's connection cap.
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

const BULK_KEYS = {
  pinned: ["history.toast.pinned", "history.toast.pinnedPartial"],
  unpinned: ["history.toast.unpinned", "history.toast.unpinnedPartial"],
  deleted: ["history.toast.bulkDeleted", "history.toast.bulkDeletedPartial"],
} as const;

function report(verb: keyof typeof BULK_KEYS, { done, failed }: BulkOutcome) {
  const [whole, partial] = BULK_KEYS[verb];
  if (failed === 0) {
    toast.success(t(whole, { count: done }));
  } else {
    toast.warning(t(partial, { done, total: done + failed, failed }));
  }
}

/** `pinned` is decided by the caller from whether *every* selected item is
 *  already pinned, so the button is one toggle whose label matches what it will
 *  do (CopyPaste-8ebg.55). */
export function useBulkPin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ items, pinned }: { items: readonly Item[]; pinned: boolean }) =>
      runBulk(items, (item) => setPinned(item.id, pinned)),
    onSuccess: (outcome, { pinned }) => {
      report(pinned ? "pinned" : "unpinned", outcome);
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
      report("deleted", outcome);
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/**
 * Not optimistic, though §3.1.6 specifies an optimistic reorder with a revert:
 * while the bridge refuses the verb, an optimistic move would show the user a
 * new order and take it away, which reads as the app losing their change
 * rather than as a missing feature.
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
