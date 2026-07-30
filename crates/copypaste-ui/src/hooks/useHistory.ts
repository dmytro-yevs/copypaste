/**
 * React Query stands in for v1's hand-written polling, which is what manifest
 * §9.1 asks for; the contracts it has to keep are INV-2 (an idle poll must not
 * produce a new array identity), INV-3 (every mutation invalidates), INV-4 (a
 * poll must not replace the merged list with page 1), INV-27 (a hidden window
 * polls zero times) and INV-33 (last write wins). §5.5 says verify rather than
 * assume — `useHistory.test.tsx` does.
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
  SEARCH_LIMIT,
  STATUS_POLL_MS,
} from "@/lib/layout";
import {
  type Item,
  type StatusData,
  copyItem,
  deleteAll,
  getStatus,
  listItems,
  searchItems,
  setPinned,
} from "@/lib/ipc";

export const HISTORY_KEY = ["history"] as const;
export const STATUS_KEY = ["status"] as const;

export const historyKey = (query: string) => [...HISTORY_KEY, query] as const;

/**
 * De-duplicated by id: a page offset is a position in a list a capture can
 * prepend to between two fetches, so one item can arrive on two pages (INV-4).
 * Module scope, because React Query memoises `select` on (data, selectFn) and
 * INV-2 needs the same array back when the data has not changed.
 */
function flatten(data: { pages: Item[][] }): readonly Item[] {
  if (data.pages.length === 1) return data.pages[0] ?? [];
  const seen = new Set<string>();
  const out: Item[] = [];
  for (const page of data.pages) {
    for (const item of page) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      out.push(item);
    }
  }
  return out;
}

/**
 * Search is deliberately **not** paged: `search` runs against the whole
 * database, so a match at index 800 is found without loading 800 rows first
 * (AT-73 / CopyPaste-crh3.106). Sensitive items are never indexed, so a search
 * legitimately cannot return one.
 */
export function useHistory(query: string) {
  const searching = query.length > 0;

  return useInfiniteQuery({
    queryKey: historyKey(query),
    queryFn: ({ pageParam }) =>
      searching ? searchItems(query, SEARCH_LIMIT) : listItems(PAGE_SIZE, pageParam),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      if (searching) return undefined;
      if (lastPage.length < PAGE_SIZE) return undefined;
      return allPages.reduce((total, page) => total + page.length, 0);
    },
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : POLL_ACTIVE_MS,
    // Typing must not blank the list between keystrokes.
    placeholderData: keepPreviousData,
    select: flatten,
  });
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
 */
export function useCopy() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => copyItem(item.id),
    onSuccess: () => {
      toast.success("Copied to clipboard", { duration: 2500 });
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
