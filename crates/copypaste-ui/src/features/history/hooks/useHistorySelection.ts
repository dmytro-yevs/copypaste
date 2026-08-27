import { useCallback, useState } from "react";

import {
  useBulkDelete,
  useBulkPin,
} from "@/features/history/hooks/useHistoryMutations";
import {
  type Selection,
  useSelection,
} from "@/features/history/hooks/useSelection";
import type { Item } from "@/lib/ipc";

export interface HistorySelection {
  readonly selection: Selection;
  /** A bulk run is in flight, so the bar's controls are inert. */
  readonly busy: boolean;
  readonly togglePin: () => void;
  readonly confirmingDelete: boolean;
  readonly requestDelete: () => void;
  readonly cancelDelete: () => void;
  readonly confirmDelete: () => void;
}

export function useHistorySelection(
  items: readonly Item[],
): HistorySelection {
  const selection = useSelection(items);
  const bulkPin = useBulkPin();
  const bulkDelete = useBulkDelete();
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const togglePin = useCallback(() => {
    const selected = selection.items;
    const pinned = !selection.allPinned;
    void bulkPin
      .mutateAsync({ items: selected, pinned })
      .then((outcome) => {
        if (outcome.failedIds.length === 0) selection.clear();
        else selection.replace(outcome.failedIds);
      })
      .catch(() => undefined);
  }, [bulkPin, selection]);

  const confirmDelete = useCallback(() => {
    const selected = selection.items;
    void bulkDelete
      .mutateAsync(selected)
      .then((outcome) => {
        if (outcome.failedIds.length === 0) selection.clear();
        else selection.replace(outcome.failedIds);
      })
      .catch(() => undefined)
      .finally(() => setConfirmingDelete(false));
  }, [bulkDelete, selection]);

  return {
    selection,
    busy: bulkPin.isPending || bulkDelete.isPending,
    togglePin,
    confirmingDelete,
    requestDelete: useCallback(() => setConfirmingDelete(true), []),
    cancelDelete: useCallback(() => setConfirmingDelete(false), []),
    confirmDelete,
  };
}
