/**
 * The history data layer: one paged query, one status query, three mutations.
 *
 * v1 carried roughly a thousand lines of hand-written polling — per-screen
 * `setInterval`s, `visibilitychange` wiring, in-flight guards, `useRef` state
 * mirrors, request sequence tags and a per-item signature hash — to satisfy
 * INV-2/3/4/27/33. Manifest §9.1 says replace the mechanism and keep the
 * contract. React Query provides:
 *
 *   INV-2  (no new reference)     structuralSharing, on by default, plus a
 *                                 module-level `select` so the flattened list
 *                                 keeps its identity across an idle poll
 *   INV-3  (mutations invalidate) invalidateQueries after every write
 *   INV-4  (load-more merges)     useInfiniteQuery refetches every loaded page,
 *                                 so a poll can never replace the merged list
 *                                 with page 1
 *   INV-27 (visibility gating)    refetchIntervalInBackground: false, plus
 *                                 refetchOnWindowFocus for the immediate
 *                                 refresh (v5's focusManager listens on
 *                                 visibilitychange)
 *   INV-33 (late responses)       built-in last-write-wins per query key
 *   INV-34 (retry policy)         retry/retryDelay, configured in main.tsx
 *
 * The numbers are the earned ones from manifest §5.1 and are the part that does
 * not come for free.
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
 * Flatten the loaded pages into the list the UI renders, de-duplicated by id.
 *
 * De-duplication is INV-4's other half: page offsets are positions in a list
 * that a capture can prepend to between two fetches, so the same item can come
 * back on two pages. Defined at module scope so its identity is stable —
 * React Query memoises `select` on (data, selectFn), and INV-2 depends on this
 * returning the *same array* when the data has not changed.
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
 * One query drives the list in both modes. An empty query pages through
 * `list`; a non-empty one runs the daemon's FTS.
 *
 * Search is deliberately **not** paged: `search` runs against the whole
 * database and returns up to `SEARCH_LIMIT`, so a match at index 800 is found
 * without the client loading 800 rows first. That is the same requirement as
 * AT-73 (CopyPaste-crh3.106), reached by asking the daemon instead of by
 * driving load-more in a loop.
 *
 * Sensitive items are never indexed, so a search legitimately cannot return
 * them (manifest 03, CLAUDE.md rule 4).
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
    // §5.1: 3000ms healthy, 5000ms while erroring.
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
 * Copy is not optimistic. The daemon re-sorts — it bumps the item's time, and
 * INV-31 requires a *pinned* item keep its slot rather than jump to the top —
 * so the server's order is the only correct one, and we invalidate and take it.
 * Nothing here reorders the list locally, which is what makes INV-31 true by
 * construction rather than by a special case.
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

/** Pin toggles refetch immediately so the server's re-sort is reflected
 *  (manifest §3.1.6). */
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

/**
 * Clear history. Destructive and un-undoable, so the caller must gate it behind
 * a confirm dialog (manifest §3.1.11; bugs 5j9x / kayk / fjvz / vcnv / w6xc all
 * come from destructive actions that were one misclick away).
 */
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
