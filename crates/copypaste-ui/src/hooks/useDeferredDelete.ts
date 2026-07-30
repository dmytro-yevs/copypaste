/**
 * Delete with an undo window — manifest §3.1.8, and CLAUDE.md rule 4 ("data
 * loss is the worst outcome").
 *
 * The row disappears immediately; the IPC fires 5000ms later. A second delete
 * commits the first at once, and anything still pending is committed when the
 * window goes away, so nothing is ever silently left undeleted (F11).
 *
 * `pending` is filtered out of the query result by the caller. When it is empty
 * the caller returns the query's array untouched, which is what preserves
 * INV-2's reference stability on the idle path.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { toFriendly } from "@/lib/errors";
import { truncate } from "@/lib/format";
import { UNDO_WINDOW_MS } from "@/lib/layout";
import { type Item, deleteItem } from "@/lib/ipc";
import { HISTORY_KEY, STATUS_KEY } from "@/hooks/useHistory";

const NO_PENDING: ReadonlySet<string> = new Set();

function without(set: ReadonlySet<string>, id: string): ReadonlySet<string> {
  if (!set.has(id)) return set;
  const next = new Set(set);
  next.delete(id);
  return next.size === 0 ? NO_PENDING : next;
}

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
        void qc.invalidateQueries({ queryKey: STATUS_KEY });
      } catch (raw) {
        toast.error(toFriendly(raw));
      } finally {
        // INV-30: the busy flag is released whatever happened above.
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
        // A sensitive item has no content on this side of the bridge at all
        // (INV-10), so there is nothing to leak into a toast; anything else is
        // truncated to 40 characters (§3.1.8).
        description:
          item.content === null
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
