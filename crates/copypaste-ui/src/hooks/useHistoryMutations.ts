import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { IMAGE_PREVIEW_KEY, imagePreviewKey } from "@/hooks/useHistoryMedia";
import { STATUS_KEY, invalidateHistoryQueries } from "@/hooks/historyRefresh";
import {
  type Item,
  copyItem,
  deleteAll,
  deleteItem,
  reorderPinned,
  setPinned,
} from "@/lib/ipc";

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
    onSuccess: async () => {
      toast.success(t("history.toast.copied"), { duration: 2500 });
      await invalidateHistoryQueries(qc);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Refetches immediately, so the server's re-sort is what is shown. */
export function usePin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => setPinned(item.id, !item.pinned),
    onSuccess: () => invalidateHistoryQueries(qc),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Destructive and un-undoable: the caller must gate it behind a confirm
 *  dialog (5j9x, kayk, fjvz, vcnv, w6xc are all one misclick away). */
export function useClearHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => deleteAll(),
    onSuccess: async (removed) => {
      toast.success(t("history.toast.cleared", { count: removed }));
      qc.removeQueries({ queryKey: IMAGE_PREVIEW_KEY });
      await Promise.all([
        invalidateHistoryQueries(qc),
        qc.invalidateQueries({ queryKey: STATUS_KEY }),
      ]);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Partial failure is reported as partial (§3.1.9). */
export interface BulkOutcome {
  readonly done: number;
  readonly failed: number;
}

/** Sequential, not `Promise.all`: every one of these is a write to one SQLite
 *  connection behind one mutex, so concurrency buys nothing and a hundred
 *  simultaneous requests trips the daemon's connection cap. */
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
      // One failure must not abandon the rest of the selection, and the raw
      // error must not reach the view (INV-12).
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
    onSuccess: async (outcome, { pinned }) => {
      report(pinned ? "pinned" : "unpinned", outcome);
      await invalidateHistoryQueries(qc);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Bulk delete has no undo (§3.1.9), so the caller must confirm first. */
export function useBulkDelete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (items: readonly Item[]) => runBulk(items, (item) => deleteItem(item.id)),
    onSuccess: async (outcome, items) => {
      report("deleted", outcome);
      for (const item of items) {
        qc.removeQueries({ queryKey: imagePreviewKey(item.id) });
      }
      await Promise.all([
        invalidateHistoryQueries(qc),
        qc.invalidateQueries({ queryKey: STATUS_KEY }),
      ]);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export function useReorderPinned() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: readonly string[]) => reorderPinned(ids),
    onSuccess: () => invalidateHistoryQueries(qc),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
