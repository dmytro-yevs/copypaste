import { useCallback, useEffect, useRef, useState } from "react";

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

interface PinRun {
  readonly token: number;
  readonly ids: readonly string[];
  readonly retainedIds: readonly string[];
  readonly pinned: boolean;
  readonly active: boolean;
}

function sameIds(left: readonly string[], right: readonly string[]): boolean {
  const expected = new Set(right);
  return left.length === expected.size && left.every((id) => expected.has(id));
}

function unreconciledPinIds(
  items: readonly Item[],
  ids: readonly string[],
  pinned: boolean,
): string[] {
  const current = new Map(items.map((item) => [item.id, item.pinned]));
  return ids.filter((id) => current.get(id) !== pinned);
}

export function useHistorySelection(
  items: readonly Item[],
): HistorySelection {
  const selection = useSelection(items);
  const bulkPin = useBulkPin();
  const bulkDelete = useBulkDelete();
  const running = useRef(false);
  const nextPinToken = useRef(0);
  const pinRun = useRef<PinRun | null>(null);
  const currentItems = useRef(items);
  currentItems.current = items;
  const [busy, setBusy] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const finishPin = useCallback(
    (started: PinRun, reportedFailedIds: readonly string[] | null) => {
      const current = pinRun.current;
      if (current === null || current.token !== started.token) return;

      const reported =
        reportedFailedIds === null ? null : new Set(reportedFailedIds);
      const candidates =
        reported === null
          ? current.ids
          : current.ids.filter((id) => reported.has(id));
      const remaining = unreconciledPinIds(
        currentItems.current,
        candidates,
        current.pinned,
      );

      selection.reconcile(current.ids, remaining);
      pinRun.current =
        remaining.length === 0
          ? null
          : {
              ...current,
              ids: candidates,
              retainedIds: remaining,
              active: false,
            };
      running.current = false;
      setBusy(false);
    },
    [selection.reconcile],
  );

  useEffect(() => {
    const current = pinRun.current;
    if (current === null) return;
    const selectedInScope = current.ids.filter((id) =>
      selection.selected.has(id),
    );
    if (
      !current.active &&
      !sameIds(selectedInScope, current.retainedIds)
    ) {
      pinRun.current = null;
      return;
    }

    const remaining = unreconciledPinIds(items, current.ids, current.pinned);
    if (sameIds(remaining, current.retainedIds)) return;

    pinRun.current =
      !current.active && remaining.length === 0
        ? null
        : { ...current, retainedIds: remaining };
    selection.reconcile(current.ids, remaining);
  }, [busy, items, selection.reconcile, selection.selected]);

  const togglePin = useCallback(() => {
    if (running.current) return;
    const selected = selection.items;
    const pinned = !selection.allPinned;
    const ids = selected.map((item) => item.id);
    const started: PinRun = {
      token: ++nextPinToken.current,
      ids,
      retainedIds: ids,
      pinned,
      active: true,
    };
    pinRun.current = started;
    running.current = true;
    setBusy(true);
    let reportedFailedIds: readonly string[] | null = null;
    void bulkPin
      .mutateAsync({ items: selected, pinned })
      .then(
        (outcome) => {
          reportedFailedIds = outcome.failedIds;
        },
        () => undefined,
      )
      .finally(() => finishPin(started, reportedFailedIds));
  }, [bulkPin, finishPin, selection]);

  const confirmDelete = useCallback(() => {
    if (running.current) return;
    pinRun.current = null;
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
    pinRun.current = null;
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
