/**
 * Delete with an undo window (§3.1.8; AGENTS.md rule 4 — data loss is the worst
 * outcome). Rows go at once, the IPC fires 5000ms later, a second delete commits
 * the first immediately, and anything pending is committed on unmount so nothing
 * is silently left undeleted (F11).
 *
 * Undo cancels the call rather than restoring rows, and that is forced, not a
 * preference: `Store::delete` and `Store::delete_all` NULL `content_ciphertext`
 * and `nonce` in the same statement that sets `deleted = 1`, so a committed
 * delete cannot be reconstructed from the row. Deferring is the only undo that
 * needs no durable copy of the plaintext-bearing ciphertext.
 */
import { createElement, useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { UndoCountdown } from "@/components/shared";
import { toFriendly } from "@/lib/errors";
import { type Item, deleteAll, deleteItem, historyCeiling } from "@/lib/ipc";
import {
  HISTORY_PAGES_KEY,
  STATUS_KEY,
  coalesceHistoryInvalidation,
  invalidateHistoryQueries,
} from "@/hooks/historyRefresh";
import { IMAGE_PREVIEW_KEY, imagePreviewKey } from "@/lib/imagePreviewQuery";

const NO_PENDING: ReadonlySet<string> = new Set();
const UNDO_WINDOW_MS = 5_000;

/** A clear-all hides rows this client has never loaded, so it cannot be
 *  expressed as a set of ids. `through` is the ceiling taken when the user
 *  asked; anything captured during the window is above it and survives. */
interface Batch {
  readonly ids: readonly string[];
  readonly all: boolean;
  readonly through?: number;
  readonly timer: ReturnType<typeof setTimeout>;
  readonly toastId: string | number;
}

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
  const [pendingAll, setPendingAll] = useState(false);
  const batches = useRef(new Map<number, Batch>());
  const nextToken = useRef(0);

  /**
   * Dismissing the toast is not cosmetic. A batch commits early on unmount and
   * on the next delete (§3.1.8), and the toast outlives both — leaving an Undo
   * button on screen that silently does nothing when pressed.
   *
   * Committing early is chosen, not accidental: only one screen is mounted at a
   * time, so leaving Settings ends a clear-all window. The data going when the
   * user believed it went is the right direction for a privacy-driven clear.
   */
  const drop = useCallback((token: number) => {
    const batch = batches.current.get(token);
    if (batch !== undefined) {
      clearTimeout(batch.timer);
      batches.current.delete(token);
      toast.dismiss(batch.toastId);
    }
    return batch;
  }, []);

  /**
   * §3.1.8 commits the previous batch as soon as the next delete is made, so
   * clearing a handful of rows is a run of small commits a few hundred
   * milliseconds apart — not one batch. Batching the *ids* therefore coalesces
   * nothing; batching the invalidation is what stops each of them re-walking
   * every loaded page on its own.
   */
  const commitIds = useCallback(
    async (ids: readonly string[]) => {
      if (ids.length === 0) return;
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
          const errored = qc
            .getQueryCache()
            .findAll({ queryKey: HISTORY_PAGES_KEY })
            .some((q) => q.state.status === "error");
          if (errored) return;
        }
      } catch {
        return;
      }
      setPending((prev) => without(prev, ids));
    },
    [qc],
  );

  const commitAll = useCallback(async (through?: number) => {
    try {
      const removed = await deleteAll(through);
      toast.success(t("history.toast.cleared", { count: removed }));
      qc.removeQueries({ queryKey: IMAGE_PREVIEW_KEY });
      await Promise.all([
        invalidateHistoryQueries(qc),
        qc.invalidateQueries({ queryKey: STATUS_KEY }),
      ]);
    } catch (raw) {
      toast.error(toFriendly(raw));
    }
    setPendingAll(false);
  }, [qc]);

  const commit = useCallback(
    (token: number) => {
      const batch = drop(token);
      if (batch === undefined) return;
      if (batch.all) void commitAll(batch.through);
      else void commitIds(batch.ids);
    },
    [commitAll, commitIds, drop],
  );

  const flush = useCallback(() => {
    for (const token of [...batches.current.keys()]) commit(token);
  }, [commit]);

  const undo = useCallback(
    (token: number) => {
      const batch = drop(token);
      if (batch === undefined) return;
      if (batch.all) setPendingAll(false);
      else setPending((prev) => without(prev, batch.ids));
      void invalidateHistoryQueries(qc);
    },
    [drop, qc],
  );

  /** One toast per batch, so undoing a selection is one gesture rather than one
   *  per row. */
  const schedule = useCallback(
    (ids: readonly string[], all: boolean, message: string, through?: number) => {
      flush();
      const token = nextToken.current++;
      if (all) setPendingAll(true);
      else setPending((prev) => new Set([...prev, ...ids]));
      const toastId = toast(message, {
        duration: UNDO_WINDOW_MS,
        description: createElement(UndoCountdown, { ms: UNDO_WINDOW_MS }),
        action: { label: t("history.toast.undo"), onClick: () => undo(token) },
      });
      batches.current.set(token, {
        ids,
        all,
        through,
        timer: setTimeout(() => commit(token), UNDO_WINDOW_MS),
        toastId,
      });
    },
    [commit, flush, undo],
  );

  const remove = useCallback(
    (item: Item) => schedule([item.id], false, t("history.toast.deleted")),
    [schedule],
  );

  const removeMany = useCallback(
    (items: readonly Item[]) => {
      if (items.length === 0) return;
      const ids = items.map((item) => item.id);
      schedule(ids, false, t("history.toast.bulkDeleted", { count: ids.length }));
    },
    [schedule],
  );

  // The ceiling is read before the window opens, not when it closes: it has to
  // name what the user was looking at when they asked.
  const removeAll = useCallback(async () => {
    let through: number | undefined;
    try {
      through = await historyCeiling();
    } catch (raw) {
      toast.error(toFriendly(raw));
      return;
    }
    schedule([], true, t("history.toast.clearing"), through);
  }, [schedule]);

  // Commit directly, without touching React state: an unmount must not turn a
  // pending delete into a silent no-op.
  useEffect(() => {
    const commitAllNow = () => {
      for (const batch of batches.current.values()) {
        clearTimeout(batch.timer);
        toast.dismiss(batch.toastId);
        if (batch.all) void deleteAll(batch.through).catch(() => {});
        else for (const id of batch.ids) void deleteItem(id).catch(() => {});
      }
      batches.current.clear();
    };
    window.addEventListener("pagehide", commitAllNow);
    return () => {
      window.removeEventListener("pagehide", commitAllNow);
      commitAllNow();
    };
  }, []);

  return { pending, pendingAll, remove, removeMany, removeAll, flush };
}
