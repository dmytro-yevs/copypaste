import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";
import { toast } from "@/lib/notify";

import { t } from "@/i18n";
import { clipboardCopyPresentation } from "@/features/history/model/clipPresentation";
import { requireClipboardWriteAvailability } from "@/hooks/useClipboardWriteAvailability";
import { acceleratorLabel } from "@/lib/accelerator";
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
import { imagePreviewKey } from "@/lib/imagePreviewQuery";

export function useCopy() {
  const qc = useQueryClient();
  const inFlight = useRef(false);
  const [isPending, setIsPending] = useState(false);
  const mutation = useMutation({
    mutationFn: async (item: Item) => {
      let availability;
      try {
        availability = await requireClipboardWriteAvailability(qc, item.content_type);
      } catch {
        throw new CopyAvailabilityFailure("failed");
      }
      if (availability !== "available") {
        throw new CopyAvailabilityFailure(availability);
      }
      return copyItem(item.id);
    },
    onSuccess: async () => {
      const shortcut = acceleratorLabel("CmdOrCtrl+V");
      toast.success(shortcut
        ? t("history.toast.copied", { shortcut })
        : t("history.toast.copiedGeneric"), { duration: 2500 });
      await invalidateHistoryQueries(qc);
    },
    onError: (raw) => {
      if (raw instanceof CopyAvailabilityFailure) {
        toast.error(clipboardCopyPresentation(
          raw.reason === "failed"
            ? { status: "failed" }
            : { status: "resolved", availability: raw.reason },
        ).reason ?? t("history.copyAvailability.failed"));
      } else {
        toast.error(toFriendly(raw));
      }
    },
  });
  const mutateAsync = useCallback((item: Item) => {
    if (inFlight.current) return Promise.reject(new Error("Clipboard write already in progress"));
    inFlight.current = true;
    setIsPending(true);
    return mutation.mutateAsync(item).finally(() => {
      inFlight.current = false;
      setIsPending(false);
    });
  }, [mutation.mutateAsync]);
  const mutate = useCallback((item: Item) => {
    void mutateAsync(item).catch(() => undefined);
  }, [mutateAsync]);
  return { mutate, mutateAsync, isPending };
}

class CopyAvailabilityFailure extends Error {
  constructor(readonly reason: "failed" | "unsupported_content_type" | "unsupported_on_platform") {
    super("Clipboard write unavailable");
  }
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
