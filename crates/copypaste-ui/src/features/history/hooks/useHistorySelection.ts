import { useCallback, useState } from "react";

import { useBulkDelete, useBulkPin } from "@/features/history/hooks/useHistoryMutations";
import { type Selection, useSelection } from "@/features/history/hooks/useSelection";
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
    bulkPin.mutate(
      { items: selection.items, pinned: !selection.allPinned },
      {
        onSuccess: (outcome) => selection.replace(outcome.failedIds),
      },
    );
  }, [bulkPin, selection]);

  const confirmDelete = useCallback(() => {
    bulkDelete.mutate(selection.items, {
      onSuccess: (outcome) => selection.replace(outcome.failedIds),
      onSettled: () => setConfirmingDelete(false),
    });
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
