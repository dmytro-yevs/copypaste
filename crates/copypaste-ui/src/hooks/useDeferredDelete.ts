/**
 * Delete with an undo window (§3.1.8; AGENTS.md rule 4 — data loss is the worst
 * outcome). The row goes at once, the IPC fires 5000ms later, a second delete
 * commits the first immediately, and anything pending is committed on unmount
 * so nothing is silently left undeleted (F11).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { UNDO_WINDOW_MS } from "@/lib/layout";
import { type Item, deleteItem } from "@/lib/ipc";
import {
  HISTORY_PAGES_KEY,
  STATUS_KEY,
  coalesceHistoryInvalidation,
  invalidateHistoryQueries,
} from "@/hooks/historyRefresh";
import { imagePreviewKey } from "@/hooks/useHistoryMedia";

const NO_PENDING: ReadonlySet<string> = new Set();

function without(
  set: ReadonlySet<string>,
  ids: readonly string[],
): ReadonlySet<string> {
  if (!ids.some((id) => set.has(id))) return set;
  const next = new Set(set);
  for (const id of ids) next.delete(id);
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

  /**
   * §3.1.8 commits the previous row's delete as soon as the next one is
   * deleted, so clearing a handful of rows is a run of one-id commits a few
   * hundred milliseconds apart — not one batch. Batching the *ids* therefore
   * coalesces nothing; batching the invalidation is what stops each of them
   * re-walking every loaded page on its own.
   */
  const commit = useCallback(
    async (ids: readonly string[]) => {
      if (ids.length === 0) return;
      for (const id of ids) clearTimer(id);
      try {
        let deleted = false;
        for (const id of ids) {
          try {
            await deleteItem(id);
            // A cached thumbnail must not outlive the clipping it was made
            // from.
            qc.removeQueries({ queryKey: imagePreviewKey(id) });
            deleted = true;
          } catch (raw) {
            toast.error(toFriendly(raw));
          }
        }
        if (deleted) {
          await coalesceHistoryInvalidation(qc);
          void qc.invalidateQueries({ queryKey: STATUS_KEY });
          // B3/INV-3: release only after a proven successful post-delete read.
          // React Query preserves stale data on error, so a failed refresh
          // would expose the cached row the delete just removed.
          const errored = qc.getQueryCache()
            .findAll({ queryKey: HISTORY_PAGES_KEY })
            .some((q) => q.state.status === "error");
          if (errored) return;
        }
      } catch {
        return;
      }
      setPending((prev) => without(prev, ids));
    },
    [clearTimer, qc],
  );

  const flush = useCallback(() => {
    void commit([...timers.current.keys()]);
  }, [commit]);

  const undo = useCallback(
    (id: string) => {
      clearTimer(id);
      setPending((prev) => without(prev, [id]));
      void invalidateHistoryQueries(qc);
    },
    [clearTimer, qc],
  );

  const remove = useCallback(
    (item: Item) => {
      flush();
      setPending((prev) => new Set(prev).add(item.id));
      timers.current.set(
        item.id,
        setTimeout(() => void commit([item.id]), UNDO_WINDOW_MS),
      );
      toast(t("history.toast.deleted"), {
        duration: UNDO_WINDOW_MS,
        action: { label: t("history.toast.undo"), onClick: () => undo(item.id) },
      });
    },
    [commit, flush, undo],
  );

  // Commit directly, without touching React state: an unmount must not turn a
  // pending delete into a silent no-op.
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
