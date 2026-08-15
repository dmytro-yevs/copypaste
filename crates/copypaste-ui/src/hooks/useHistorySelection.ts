import { useCallback, useState } from "react";

import { useBulkPin } from "@/hooks/useHistoryMutations";
import { type Selection, useSelection } from "@/hooks/useSelection";
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

/** The confirmation stays even though bulk delete is now undoable (DMY-168):
 *  the undo window is five seconds and the blast radius is the whole selection,
 *  so the two guards answer different risks. */
export function useHistorySelection(
  items: readonly Item[],
  removeMany: (items: readonly Item[]) => void,
): HistorySelection {
  const selection = useSelection(items);
  const bulkPin = useBulkPin();
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const togglePin = useCallback(() => {
    bulkPin.mutate(
      { items: selection.items, pinned: !selection.allPinned },
      { onSettled: () => selection.end() },
    );
  }, [bulkPin, selection]);

  const confirmDelete = useCallback(() => {
    removeMany(selection.items);
    selection.end();
    setConfirmingDelete(false);
  }, [removeMany, selection]);

  return {
    selection,
    busy: bulkPin.isPending,
    togglePin,
    confirmingDelete,
    requestDelete: useCallback(() => setConfirmingDelete(true), []),
    cancelDelete: useCallback(() => setConfirmingDelete(false), []),
    confirmDelete,
  };
}
