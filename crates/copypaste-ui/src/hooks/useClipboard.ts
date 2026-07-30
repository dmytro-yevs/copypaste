/**
 * The whole data layer: two queries and three mutations.
 *
 * v1 carried roughly a thousand lines of hand-written polling — per-screen
 * `setInterval`s, `visibilitychange` wiring, in-flight guards, `useRef` state
 * mirrors, request sequence tags and a per-item signature hash — to satisfy
 * INV-2/3/4/27/33. Manifest §9.1 says replace the mechanism and keep the
 * contract. React Query provides:
 *
 *   INV-27 (visibility gating)  refetchIntervalInBackground: false, plus
 *                               refetchOnWindowFocus for the immediate refresh
 *                               (v5's focusManager listens on visibilitychange)
 *   INV-2  (no new reference)   structuralSharing, on by default
 *   INV-3  (mutations invalidate) invalidateQueries after every write
 *   INV-33 (late responses)     built-in last-write-wins per query key
 *   INV-34 (retry policy)       retry/retryDelay, configured in main.tsx
 *
 * The numbers below are the earned ones from manifest §5.1 and are the part
 * that does not come for free.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";

import { toFriendly } from "../lib/errors";
import { truncate } from "../lib/format";
import { PAGE_SIZE, SEARCH_LIMIT, UNDO_WINDOW_MS } from "../lib/layout";
import {
  type Item,
  type StatusData,
  copyItem,
  deleteItem,
  getStatus,
  listItems,
  searchItems,
  setPinned,
} from "../lib/ipc";

/** Manifest §5.1: 3000ms while healthy, 5000ms while offline / erroring —
 *  never hammer a dead or initialising service. */
const POLL_ACTIVE_MS = 3000;
const POLL_BACKOFF_MS = 5000;
/** §5.1: the status chip polls faster so "offline" surfaces within a cycle. */
const STATUS_POLL_MS = 2000;

export const HISTORY_KEY = ["history"] as const;
export const STATUS_KEY = ["status"] as const;

/* ------------------------------------------------------------- queries --- */

/**
 * One query drives the list in both modes. An empty query lists; a non-empty
 * one runs the daemon's FTS. Keeping them on one key means the poll cadence,
 * the error handling and the scroll anchoring have a single source.
 *
 * Sensitive items are never indexed, so a search legitimately cannot return
 * them (manifest 03, CLAUDE.md rule 4).
 */
export function useHistory(query: string) {
  return useQuery({
    queryKey: [...HISTORY_KEY, query],
    queryFn: () =>
      query ? searchItems(query, SEARCH_LIMIT) : listItems(PAGE_SIZE, 0),
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : POLL_ACTIVE_MS,
    // Typing must not blank the list between keystrokes.
    placeholderData: keepPreviousData,
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
 * Copy is not optimistic. The daemon re-sorts (it bumps the item's time, and
 * INV-31 requires that a *pinned* item keeps its slot rather than jumping to
 * the top), so the server's order is the only correct one — we invalidate and
 * take it.
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

/* ------------------------------------------------------ deferred delete --- */

const NO_PENDING: ReadonlySet<string> = new Set();

function without(set: ReadonlySet<string>, id: string): ReadonlySet<string> {
  if (!set.has(id)) return set;
  const next = new Set(set);
  next.delete(id);
  return next.size === 0 ? NO_PENDING : next;
}

/**
 * Delete with an undo window — manifest §3.1.8, and CLAUDE.md rule 4 ("data
 * loss is the worst outcome"). The row disappears immediately; the IPC fires
 * 5000ms later. A second delete commits the first at once, and anything still
 * pending is committed when the window goes away, so nothing is silently left
 * undeleted.
 *
 * `pending` is filtered out of the query result by the caller. When it is empty
 * the caller returns the query's array untouched, which is what preserves
 * INV-2's reference stability on the idle path.
 */
export function useDeferredDelete() {
  const qc = useQueryClient();
  const [pending, setPending] = useState<ReadonlySet<string>>(NO_PENDING);
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  const clearTimer = useCallback((id: string) => {
    const timer = timers.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.current.delete(id);
    }
  }, []);

  const commit = useCallback(
    async (id: string) => {
      clearTimer(id);
      try {
        await deleteItem(id);
        // INV-3: the write invalidates, and we only stop hiding the row once
        // the refetch has settled — otherwise it flashes back into the list.
        await qc.invalidateQueries({ queryKey: HISTORY_KEY });
      } catch (raw) {
        toast.error(toFriendly(raw));
      } finally {
        setPending((prev) => without(prev, id));
      }
    },
    [clearTimer, qc],
  );

  const flush = useCallback(() => {
    for (const id of [...timers.current.keys()]) void commit(id);
  }, [commit]);

  const undo = useCallback(
    (id: string) => {
      clearTimer(id);
      setPending((prev) => without(prev, id));
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    [clearTimer, qc],
  );

  const remove = useCallback(
    (item: Item) => {
      flush();
      setPending((prev) => new Set(prev).add(item.id));
      timers.current.set(
        item.id,
        setTimeout(() => void commit(item.id), UNDO_WINDOW_MS),
      );
      toast("Deleted", {
        // Never the plaintext of a sensitive item (INV-10), and 40 characters
        // for anything else (§3.1.8).
        description: item.is_sensitive
          ? "Sensitive item"
          : truncate(item.content, 40),
        duration: UNDO_WINDOW_MS,
        action: { label: "Undo", onClick: () => undo(item.id) },
      });
    },
    [commit, flush, undo],
  );

  // A pending delete must survive neither a reload nor an unmount as a silent
  // no-op: commit it directly, without touching React state.
  useEffect(() => {
    const commitAllNow = () => {
      for (const [id, timer] of timers.current) {
        clearTimeout(timer);
        void deleteItem(id).catch(() => {});
      }
      timers.current.clear();
    };
    window.addEventListener("pagehide", commitAllNow);
    return () => {
      window.removeEventListener("pagehide", commitAllNow);
      commitAllNow();
    };
  }, []);

  return { pending, remove };
}
