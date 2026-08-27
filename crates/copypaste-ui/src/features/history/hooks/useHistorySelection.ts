import { useCallback, useRef, useState } from "react";

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
  readonly end: () => void;
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
  const running = useRef(false);
  const [busy, setBusy] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const togglePin = useCallback(() => {
    if (running.current) return;
    const selected = selection.items;
    const pinned = !selection.allPinned;
    running.current = true;
    setBusy(true);
    void bulkPin
      .mutateAsync({ items: selected, pinned })
      .then((outcome) => selection.replace(outcome.failedIds))
      .catch(() => undefined)
      .finally(() => {
        running.current = false;
        setBusy(false);
      });
  }, [bulkPin, selection]);

  const confirmDelete = useCallback(() => {
    if (running.current) return;
    const selected = selection.items;
    running.current = true;
    setBusy(true);
    void bulkDelete
      .mutateAsync(selected)
      .then((outcome) => {
        if (outcome.failedIds.length === 0) selection.clear();
        else selection.replace(outcome.failedIds);
      })
      .catch(() => undefined)
      .finally(() => {
        running.current = false;
        setBusy(false);
        setConfirmingDelete(false);
      });
  }, [bulkDelete, selection]);

  const end = useCallback(() => {
    if (running.current) return;
    setConfirmingDelete(false);
    selection.clear();
  }, [selection.clear]);

  return {
    selection,
    busy,
    end,
    togglePin,
    confirmingDelete,
    requestDelete: useCallback(() => setConfirmingDelete(true), []),
    cancelDelete: useCallback(() => setConfirmingDelete(false), []),
    confirmDelete,
  };
}
