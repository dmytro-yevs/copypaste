import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import {
  coalesceHistoryInvalidation,
  invalidateHistoryQueries,
  STATUS_KEY,
} from "@/hooks/historyRefresh";
import {
  type Item,
  copyItem,
  deleteItem,
  reorderPinned,
  setPinned,
} from "@/lib/ipc";
import { imagePreviewKey } from "@/features/history/model";

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

export function usePin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (item: Item) => setPinned(item.id, !item.pinned),
    onSuccess: () => invalidateHistoryQueries(qc),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export interface BulkOutcome {
  readonly done: number;
  readonly failedIds: readonly string[];
}

async function runBulk(
  targets: readonly Item[],
  each: (target: Item) => Promise<unknown>,
): Promise<BulkOutcome> {
  let done = 0;
  const failedIds: string[] = [];
  for (const target of targets) {
    try {
      await each(target);
      done += 1;
    } catch {
      failedIds.push(target.id);
    }
  }
  return { done, failedIds };
}

const BULK_KEYS = {
  pinned: ["history.toast.pinned", "history.toast.pinnedPartial"],
  unpinned: ["history.toast.unpinned", "history.toast.unpinnedPartial"],
  deleted: ["history.toast.bulkDeleted", "history.toast.bulkDeletedPartial"],
} as const;

function report(verb: keyof typeof BULK_KEYS, outcome: BulkOutcome) {
  const [whole, partial] = BULK_KEYS[verb];
  const failed = outcome.failedIds.length;
  if (failed === 0) toast.success(t(whole, { count: outcome.done }));
  else toast.warning(t(partial, { done: outcome.done, total: outcome.done + failed, failed }));
}

export function useBulkPin() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ items, pinned }: { items: readonly Item[]; pinned: boolean }) =>
      runBulk(items, (item) => setPinned(item.id, pinned)),
    onSuccess: (outcome, { pinned }) => {
      report(pinned ? "pinned" : "unpinned", outcome);
    },
    onError: (raw) => toast.error(toFriendly(raw)),
    onSettled: () => {
      void invalidateHistoryQueries(qc);
    },
  });
}

export function useBulkDelete() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (items: readonly Item[]) => runBulk(items, (item) => deleteItem(item.id)),
    onSuccess: async (outcome, items) => {
      report("deleted", outcome);
      const failed = new Set(outcome.failedIds);
      for (const item of items) {
        if (!failed.has(item.id)) qc.removeQueries({ queryKey: imagePreviewKey(item.id) });
      }
      await coalesceHistoryInvalidation(qc);
      void qc.invalidateQueries({ queryKey: STATUS_KEY });
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
