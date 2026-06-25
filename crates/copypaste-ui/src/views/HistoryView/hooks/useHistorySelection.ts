/**
 * useHistorySelection — multi-select state and helpers for HistoryView.
 *
 * Extracted from HistoryView.tsx (CopyPaste-g06m.34 refactor).
 * Owns: selectionMode, multiSelectedIds, bulkBusy, clearSelection,
 *       toggleMultiSelect, selectAll, allSelected.
 */
import { useCallback, useEffect, useState } from "react";
import { type HistoryEntry } from "../../../lib/ipc";

export function useHistorySelection(filtered: HistoryEntry[]) {
  // ---------------------------------------------------------------------------
  // Multi-select state
  // selectionMode: checkbox column is visible + bulk bar is shown
  // multiSelectedIds: Set of item ids checked in the bulk-select UI
  // bulkBusy: true while a bulk operation is in flight (disables buttons)
  // ---------------------------------------------------------------------------
  const [selectionMode, setSelectionMode] = useState(false);
  const [multiSelectedIds, setMultiSelectedIds] = useState<Set<string>>(new Set());
  const [bulkBusy, setBulkBusy] = useState(false);

  // Exit selection mode automatically when the last item is deselected.
  // A useEffect watching the set size is race-free: it runs after React has
  // committed the new multiSelectedIds state, so a concurrent toggleMultiSelect
  // that re-adds an item before the effect fires will see size > 0 and won't
  // flip selectionMode off.  The old Promise.resolve().then() micro-task hack
  // ran before the next render and could interleave with a concurrent select.
  useEffect(() => {
    if (selectionMode && multiSelectedIds.size === 0) {
      setSelectionMode(false);
    }
  }, [selectionMode, multiSelectedIds]);

  /** Exit selection mode and clear all multi-select state. */
  const clearSelection = useCallback(() => {
    setSelectionMode(false);
    setMultiSelectedIds(new Set());
  }, []);

  /** Toggle a single item's multi-select state; activates selection mode on first check. */
  const toggleMultiSelect = useCallback((id: string) => {
    setSelectionMode(true);
    setMultiSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  /** Select all currently-visible (filtered) items. */
  const selectAll = useCallback(() => {
    setSelectionMode(true);
    setMultiSelectedIds(new Set(filtered.map((it) => it.id)));
  }, [filtered]);

  const allSelected =
    filtered.length > 0 && filtered.every((it) => multiSelectedIds.has(it.id));

  return {
    selectionMode,
    setSelectionMode,
    multiSelectedIds,
    setMultiSelectedIds,
    bulkBusy,
    setBulkBusy,
    clearSelection,
    toggleMultiSelect,
    selectAll,
    allSelected,
  };
}
