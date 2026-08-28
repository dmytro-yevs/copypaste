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
  readonly pinned: boolean;
  readonly active: boolean;
}

function sameIds(left: ReadonlySet<string>, right: readonly string[]): boolean {
  return left.size === right.length && right.every((id) => left.has(id));
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

      selection.replace(remaining);
      pinRun.current =
        remaining.length === 0
          ? null
          : { ...current, ids: remaining, active: false };
      running.current = false;
      setBusy(false);
    },
    [selection.replace],
  );

  useEffect(() => {
    const current = pinRun.current;
    if (current === null) return;
    if (!current.active && !sameIds(selection.selected, current.ids)) {
      pinRun.current = null;
      return;
    }

    const remaining = unreconciledPinIds(items, current.ids, current.pinned);
    if (remaining.length === current.ids.length) return;

    pinRun.current = { ...current, ids: remaining };
    selection.replace(remaining);
  }, [busy, items, selection.replace, selection.selected]);

  const togglePin = useCallback(() => {
    if (running.current) return;
    const selected = selection.items;
    const pinned = !selection.allPinned;
    const started: PinRun = {
      token: ++nextPinToken.current,
      ids: selected.map((item) => item.id),
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
