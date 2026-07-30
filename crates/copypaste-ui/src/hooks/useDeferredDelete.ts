/**
 * Delete with an undo window (§3.1.8; CLAUDE.md rule 4 — data loss is the worst
 * outcome). The row goes at once, the IPC fires 5000ms later, a second delete
 * commits the first immediately, and anything pending is committed on unmount
 * so nothing is silently left undeleted (F11).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
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
        // INV-30: released whatever happened above.
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
      toast(t("history.toast.deleted"), {
        // A sensitive item has no content on this side of the bridge at all
        // (INV-10), so there is nothing to leak into a toast; anything else is
        // truncated to 40 characters (§3.1.8) and passed as a value, never
        // through a message template.
        description:
          item.content === null
            ? t("history.toast.deletedSensitive")
            : truncate(item.content, 40),
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
