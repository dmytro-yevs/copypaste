import React, { useCallback, useEffect, useRef, useState } from "react";
// h97m: listen for cross-view "history-refresh" events emitted after a
// successful backup import so HistoryView re-fetches immediately.
import { ViewShell } from "../components/ViewShell";
import {
  api,
  ipcErrorMessage,
  friendlyIpcError,
  isImageType,
  pasteAsPlainText,
  playCopySound,
  resetDatabase,
  showCopyNotification,
  type HistoryEntry,
} from "../lib/ipc";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { EmptyState } from "../components/EmptyState";
import { useUI } from "../store";
import { SKINS } from "../lib/skins";
import { clearImageCache } from "../components/ImageThumb";
import { ConfirmModal } from "../components/ConfirmModal";
// CopyPaste-5917.102: replaced the local Toast duplicate with the shared
// GlassToast system. useToast() wires all showToast() calls to the provider;
// ToastProvider is mounted as a self-contained wrapper inside HistoryView's
// return so no App-level changes are needed.
import { useToast, ToastProvider, type ToastKind } from "../components/Toast";
// CopyPaste-bdac.23: ActionButton is used in BulkActionBar (HistoryView/BulkActionBar.tsx).

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// itemsSignature / _itemsSigCache extracted to HistoryView/historySignature.ts
// Re-exported here so existing importers ("./HistoryView") keep working.
export { itemsSignature, _itemsSigCache } from "./HistoryView/historySignature";

// HistoryRow + PinIndicator + SyncBlockedIndicator + Icon* micro-components + parseFilename/parseUrl
// extracted to HistoryView/HistoryRow.tsx (CopyPaste-g06m.13 refactor).
import { HistoryRow } from "./HistoryView/HistoryRow";

// ---------------------------------------------------------------------------
// Virtualizer pure fns — extracted to HistoryView/historyVirtualizer.ts
// Re-exported here so existing importers ("./HistoryView") keep working.
// ---------------------------------------------------------------------------
export { rowHeightFor, buildOffsets, computeVisibleWindow } from "./HistoryView/historyVirtualizer";
import { rowHeightFor as _rowHeightFor } from "./HistoryView/historyVirtualizer";


// IconActionBtn removed — use imported IconActionButton (CopyPaste-bdac.26).

// ---------------------------------------------------------------------------
// Bulk action bar — extracted to HistoryView/BulkActionBar.tsx
// ---------------------------------------------------------------------------
import { BulkActionBar } from "./HistoryView/BulkActionBar";

// FullResImage + DetailsModal extracted to HistoryView/DetailsModal.tsx
import { DetailsModal } from "./HistoryView/DetailsModal";

// VirtualList + LOAD_MORE_THRESHOLD_PX extracted to HistoryView/VirtualList.tsx
import { VirtualList } from "./HistoryView/VirtualList";

// ---------------------------------------------------------------------------
// Extracted hooks (CopyPaste-g06m.34 refactor)
// ---------------------------------------------------------------------------
import { useHistoryData } from "./HistoryView/hooks/useHistoryData";
import { useHistoryFilter } from "./HistoryView/hooks/useHistoryFilter";
import { useHistorySelection } from "./HistoryView/hooks/useHistorySelection";
import { useFileDrop } from "./HistoryView/hooks/useFileDrop";

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function HistoryViewInner() {
  const { previewLinesApp, previewSize, imageMaxHeight, maskSensitive, showSensitiveWarnings, playSoundOnCopy, notifyOnCopy, density, historyDisplayLimit, skin, sortByDevice } =
    useUI((s) => s.prefs);
  const setPrefs = useUI((s) => s.setPrefs);

  // -------------------------------------------------------------------------
  // Data loading, state, and side effects
  // -------------------------------------------------------------------------
  const {
    items,
    setItems,
    ownDeviceId,
    totalCount,
    loadState,
    errorDetail,
    setErrorDetail,
    degraded,
    setDegraded,
    isPrivateMode,
    undoPending,
    setUndoPending,
    undoPendingRef,
    sigRef,
    load,
    handleNearBottom,
  } = useHistoryData();

  // -------------------------------------------------------------------------
  // Search, FTS, device filter, and sort
  // -------------------------------------------------------------------------
  const {
    search,
    setSearch,
    deviceFilter,
    setDeviceFilter,
    sortMode,
    toggleSortMode,
    knownDeviceIds,
    deviceOptionLabel,
    filtered,
  } = useHistoryFilter(items, ownDeviceId, sortByDevice, setPrefs);

  // -------------------------------------------------------------------------
  // Multi-select
  // -------------------------------------------------------------------------
  const {
    selectionMode,
    multiSelectedIds,
    bulkBusy,
    setBulkBusy,
    clearSelection,
    toggleMultiSelect,
    selectAll,
    allSelected,
  } = useHistorySelection(filtered);

  // 5j9x: modal confirm state for the destructive database reset.
  // Replaced the misclick-prone inline Yes/No with a ConfirmModal.
  const [resetConfirm, setResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);

  // kayk: "Clear all" — modal confirm + in-flight state.
  const [clearAllConfirmOpen, setClearAllConfirmOpen] = useState(false);
  const [clearAllBusy, setClearAllBusy] = useState(false);

  // M10: Details modal — entry to preview (null = closed)
  const [previewEntry, setPreviewEntry] = useState<HistoryEntry | null>(null);

  // fjvz: confirmation modal state for bulk delete.
  // true = modal is open; false = modal is closed.
  const [bulkDeleteConfirmOpen, setBulkDeleteConfirmOpen] = useState(false);

  // A1: Drag-to-reorder pinned items state
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{ id: string; position: "above" | "below" } | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const isKeyboardNavRef = useRef(false);

  // §8 Mount stagger: true only during the initial mount window (before the first
  // successful data render). Set to false after the first render completes so that
  // subsequent filter/search re-renders are instant (never re-stagger on list change).
  // Gate: a ref (not state) so setting it never causes a re-render.
  const staggerActiveRef = useRef(true);
  // Flip off on the first commit after data loads (via useEffect with no deps —
  // runs once, after the initial render is painted).
  useEffect(() => {
    // Use a rAF so the first frame renders with stagger classes, then on the
    // very next frame we mark stagger done (preventing second render from restaggering).
    const id = requestAnimationFrame(() => {
      staggerActiveRef.current = false;
    });
    return () => cancelAnimationFrame(id);
  }, []);

  // §8 Selection glide: track the pixel position + height of the selected row
  // so the absolutely-positioned glide layer can animate to it.
  // `null` = no selection (glide layer hidden).
  const [glideStyle, setGlideStyle] = useState<{ top: number; height: number } | null>(null);

  // CopyPaste-5917.102: showToast now delegates to the shared GlassToast system
  // via useToast(). The local Toast function and per-instance timer state are gone.
  const { show: _toastShow } = useToast();
  const showToast = useCallback(
    (message: string, kind: ToastKind, durationMs = 2500) => {
      _toastShow(message, { kind, duration: durationMs });
    },
    [_toastShow]
  );

  // -------------------------------------------------------------------------
  // File picker (D2) and OS drag-drop (D3)
  // -------------------------------------------------------------------------
  const { fileDragOver, fileInputRef, handleFileInputChange } = useFileDrop(load, showToast);

  const [selectedId, setSelectedId] = useState<string | null>(null);

  const selectedIdx = filtered.findIndex((it) => it.id === selectedId);

  // Keep the selected row visible. With virtualization an off-screen selected
  // row isn't in the DOM, so we compute its offset from the height model and
  // scroll the container directly instead of relying on scrollIntoView.
  useEffect(() => {
    if (!isKeyboardNavRef.current) return;
    if (selectedIdx < 0) return;
    const el = listRef.current;
    if (!el) return;
    let top = 0;
    for (let i = 0; i < selectedIdx; i++) {
      top += _rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
    }
    const rowH = _rowHeightFor(filtered[selectedIdx], previewSize, imageMaxHeight, density);
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top < viewTop) {
      el.scrollTop = top;
    } else if (top + rowH > viewBottom) {
      el.scrollTop = top + rowH - el.clientHeight;
    }
    isKeyboardNavRef.current = false;
  }, [selectedIdx, filtered, previewSize, imageMaxHeight, density]);

  // §8 Selection glide: update the glide layer position whenever selection or
  // filtered list changes. Computes the offset from rowHeightFor so it stays
  // in sync with the virtualizer's prefix-sum math.
  // Multi-select: glide covers the union of selected rows (first→last).
  useEffect(() => {
    if (selectedId === null && multiSelectedIds.size === 0) {
      setGlideStyle(null);
      return;
    }
    // Single-select path: track the selectedId row.
    if (multiSelectedIds.size === 0 && selectedId !== null) {
      const idx = filtered.findIndex((it) => it.id === selectedId);
      if (idx < 0) { setGlideStyle(null); return; }
      let top = 0;
      for (let i = 0; i < idx; i++) {
        top += _rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
      }
      const height = _rowHeightFor(filtered[idx], previewSize, imageMaxHeight, density);
      setGlideStyle({ top, height });
      return;
    }
    // CopyPaste-5917.75: multi-select path — hide the glide layer entirely.
    // The old code drew a single contiguous rectangle from the first to the last
    // selected row, which visually covered unselected interleaved rows and made
    // them appear selected. Instead, rely solely on the per-row bg-ide-selection
    // class (driven by the `multiSelected` prop on HistoryRow) to highlight only
    // the actually-selected rows.
    setGlideStyle(null);
  }, [selectedId, multiSelectedIds, filtered, previewSize, imageMaxHeight, density]);

  // Defined before handleKeyDown so the Enter-key path can route copies through
  // it (sound/notification fire on success via the same prefs as row-click copy).
  const handleCopy = useCallback(
    async (id: string) => {
      try {
        await api.copyItem(id);
        // Fire sound / notification on successful copy — same gates as the popup.
        if (playSoundOnCopy) {
          void playCopySound();
        }
        if (notifyOnCopy) {
          // Use content_type + preview from HistoryEntry for rich notification.
          const item = items.find((it) => it.id === id);
          void showCopyNotification(
            item?.content_type ?? "",
            item?.preview ?? ""
          );
        }
        // Optimistically move the copied item to the top — but only for
        // unpinned items. Pinned items keep their pin_order position; the daemon
        // only bumps wall_time, which does not affect their sort position.
        setItems((prev) => {
          const idx = prev.findIndex((it) => it.id === id);
          if (idx <= 0) return prev; // already at top or not found
          const item = prev[idx];
          if (item.pinned) {
            // Pinned items must not jump to top — let the next poll reflect
            // the server state (wall_time bump only, pin_order unchanged).
            sigRef.current = "";
            return prev;
          }
          const next = [...prev];
          next.splice(idx, 1);
          // Insert after the last pinned item so the unpinned section is correct.
          const lastPinnedIdx = next.reduce(
            (acc, it, i) => (it.pinned ? i : acc),
            -1
          );
          next.splice(lastPinnedIdx + 1, 0, item);
          sigRef.current = ""; // allow next poll to re-render with server state
          return next;
        });
        void load(true);
      } catch (err) {
        const msg = ipcErrorMessage(err, "Copy failed");
        showToast(msg, "error");
      }
    },
    [items, load, playSoundOnCopy, notifyOnCopy, showToast, setItems, sigRef]
  );

  // F11: handleDelete/handleUndo must be declared before handleKeyDown so the
  // keyboard handler can reference them without a "used before declaration" error.

  // Optimistically removes the item from local state and schedules the actual
  // api.deleteItem call after a 5-second undo window.  If a second delete fires
  // before the timer expires the first is committed immediately.
  const handleDelete = useCallback(
    (id: string, preview: string) => {
      const prev = undoPendingRef.current;
      if (prev !== null) {
        clearTimeout(prev.timer);
        void api.deleteItem(prev.id).catch(() => {});
      }
      setItems((prevItems) => prevItems.filter((it) => it.id !== id));
      if (selectedId === id) setSelectedId(null);
      const timer = setTimeout(() => {
        void api.deleteItem(id).catch(() => {});
        setUndoPending(null);
      }, 5000);
      setUndoPending({ id, preview, timer });
    },
    [selectedId, undoPendingRef, setItems, setUndoPending]
  );

  const handleUndo = useCallback(() => {
    const pending = undoPendingRef.current;
    if (pending === null) return;
    clearTimeout(pending.timer);
    setUndoPending(null);
    void load(true);
  }, [load, undoPendingRef, setUndoPending]);

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent<HTMLDivElement>) => {
      // Escape always clears multi-selection (or single selection if in selection mode).
      if (e.key === "Escape") {
        e.preventDefault();
        if (selectionMode) {
          clearSelection();
        } else {
          setSelectedId(null);
        }
        return;
      }

      // CopyPaste-5917.65: Cmd+F / Ctrl+F focuses the search input and selects any
      // existing text — matches macOS "Find" convention and Maccy's search flow.
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
        return;
      }

      // Cmd+A (or Ctrl+A on non-Mac) selects all when focused on the list.
      if ((e.metaKey || e.ctrlKey) && e.key === "a") {
        e.preventDefault();
        selectAll();
        return;
      }

      if (filtered.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        isKeyboardNavRef.current = true;
        const next = Math.min(selectedIdx + 1, filtered.length - 1);
        setSelectedId(filtered[next].id);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        isKeyboardNavRef.current = true;
        const prev = Math.max(selectedIdx - 1, 0);
        setSelectedId(filtered[prev].id);
      } else if (e.key === "Enter" && e.altKey && selectedId !== null) {
        // Option+Enter (F1): paste as plain text — strip rich formatting.
        e.preventDefault();
        try {
          const item = items.find((it) => it.id === selectedId);
          const text = item?.preview ?? "";
          await pasteAsPlainText(text);
        } catch (err) {
          console.error("paste-as-plain-text failed:", err);
        }
      } else if (e.key === "Enter" && selectedId !== null) {
        e.preventDefault();
        // Route through handleCopy so sound/notification fire on success
        // using the same playSoundOnCopy/notifyOnCopy gates as row-click copy.
        await handleCopy(selectedId);
      } else if ((e.key === "Backspace" || e.key === "Delete") && selectedId !== null) {
        e.preventDefault();
        const entry = filtered.find((it) => it.id === selectedId);
        // Select the next item before removing the current one from the list.
        const newIdx = Math.min(selectedIdx, filtered.length - 2);
        setSelectedId(newIdx >= 0 ? (filtered[newIdx]?.id ?? null) : null);
        handleDelete(selectedId, entry?.preview ?? "");
      }
    },
    [filtered, selectedIdx, selectedId, selectionMode, clearSelection, selectAll, load, showToast, handleCopy, handleDelete, items]
  );

  // -------------------------------------------------------------------------
  // Single-item actions (existing per-row behavior)
  // -------------------------------------------------------------------------

  const handlePin = useCallback(
    async (id: string, currentlyPinned: boolean) => {
      try {
        await api.pinItem(id, !currentlyPinned);
        // Immediate refresh so the server's new state + re-sort is reflected.
        void load(true);
      } catch (err) {
        const msg = ipcErrorMessage(err, "Pin failed");
        showToast(msg, "error");
      }
    },
    [load, showToast]
  );

  // A1: Drag-to-reorder handler — placed after `load` and `showToast` are declared
  const handleReorderDrop = useCallback(
    async (draggedId: string, targetId: string, position: "above" | "below") => {
      if (draggedId === targetId) return;
      // Compute new order from current pinned items list (preserve optimistic order).
      const pinnedItems = items.filter((it) => it.pinned);
      const dragIdx = pinnedItems.findIndex((it) => it.id === draggedId);
      const targetIdx = pinnedItems.findIndex((it) => it.id === targetId);
      if (dragIdx < 0 || targetIdx < 0) return;

      // Build the new ordered IDs by moving draggedId to the correct position.
      const reordered = pinnedItems.filter((it) => it.id !== draggedId);
      const insertAt = reordered.findIndex((it) => it.id === targetId);
      const finalIdx = position === "above" ? insertAt : insertAt + 1;
      reordered.splice(finalIdx, 0, pinnedItems[dragIdx]);
      const newIds = reordered.map((it) => it.id);

      // Optimistically reorder in local state so the UI responds immediately.
      setItems((prev) => {
        const pinnedById = new Map(prev.filter((it) => it.pinned).map((it) => [it.id, it]));
        const unpinned = prev.filter((it) => !it.pinned);
        const reorderedPinned = newIds.map((id) => pinnedById.get(id)!).filter(Boolean);
        return [...reorderedPinned, ...unpinned];
      });

      try {
        await api.reorderPinned(newIds);
        void load(true);
      } catch (err) {
        const msg = ipcErrorMessage(err, "Reorder failed");
        showToast(msg, "error");
        // Revert to server state on failure.
        void load(true);
      }
    },
    [items, load, showToast, setItems]
  );

  // -------------------------------------------------------------------------
  // Bulk actions — call single-item IPCs in a loop (no bulk IPC exists).
  // api.deleteItem, api.pinItem are used per-item sequentially.
  // For bulk copy we concatenate preview text of selected items (non-image,
  // non-sensitive), then write to clipboard via api.copyItem on the first
  // selected item (the daemon puts that item on the pasteboard). For a richer
  // concatenation we rely on the browser clipboard API as a fallback.
  // -------------------------------------------------------------------------

  const handleBulkDelete = useCallback(async () => {
    if (bulkBusy || multiSelectedIds.size === 0) return;
    setBulkBusy(true);
    const ids = Array.from(multiSelectedIds);
    let failed = 0;
    try {
      for (const id of ids) {
        try {
          await api.deleteItem(id);
        } catch {
          failed++;
        }
      }
      // Clear selection and refresh regardless of partial failures.
      clearSelection();
      if (selectedId !== null && multiSelectedIds.has(selectedId)) setSelectedId(null);
      sigRef.current = ""; // force re-render
      void load(true);
      if (failed > 0) {
        showToast(`Deleted ${ids.length - failed}/${ids.length} (${failed} failed)`, "error");
      } else {
        showToast(`Deleted ${ids.length} item${ids.length === 1 ? "" : "s"}`, "success");
      }
    } finally {
      // Always release the busy flag — even if clearSelection/load throws,
      // so the bulk action bar is never permanently disabled (V-13).
      setBulkBusy(false);
    }
  }, [bulkBusy, multiSelectedIds, clearSelection, selectedId, load, showToast, sigRef]);

  const handleBulkPin = useCallback(
    async (targetPinned: boolean) => {
      if (bulkBusy || multiSelectedIds.size === 0) return;
      setBulkBusy(true);
      const ids = Array.from(multiSelectedIds);
      let failed = 0;
      try {
        for (const id of ids) {
          try {
            await api.pinItem(id, targetPinned);
          } catch {
            failed++;
          }
        }
        clearSelection();
        sigRef.current = "";
        void load(true);
        const verb = targetPinned ? "Pinned" : "Unpinned";
        if (failed > 0) {
          showToast(`${verb} ${ids.length - failed}/${ids.length} (${failed} failed)`, "error");
        } else {
          showToast(`${verb} ${ids.length} item${ids.length === 1 ? "" : "s"}`, "success");
        }
      } finally {
        // Always release the busy flag — even if clearSelection/load throws,
        // so the bulk action bar is never permanently disabled (V-13).
        setBulkBusy(false);
      }
    },
    [bulkBusy, multiSelectedIds, clearSelection, load, showToast, sigRef]
  );

  /**
   * Bulk copy: copies the first selected item via daemon IPC (which puts it on
   * the pasteboard), then also writes all non-sensitive preview text joined by
   * newlines to the browser clipboard API for a richer paste target.
   * Images are excluded from the text concatenation (they have no preview text).
   */
  const handleBulkCopy = useCallback(async () => {
    if (bulkBusy || multiSelectedIds.size === 0) return;
    setBulkBusy(true);

    // Collect selected items in the current filtered order so the user gets
    // the same order they see on screen.
    const selectedItems = filtered.filter((it) => multiSelectedIds.has(it.id));

    try {
      // Step 1: copy the first selected item via daemon (puts it on pasteboard).
      const firstId = selectedItems[0]?.id;
      if (firstId !== undefined) {
        try {
          await api.copyItem(firstId);
        } catch (err) {
          const msg = ipcErrorMessage(err, "Copy failed");
          showToast(msg, "error");
          // Return inside try so finally still runs and releases the busy flag (V-13).
          return;
        }
      }

      // Step 2: if the browser clipboard API is available, write the concatenated
      // preview text of all selected non-sensitive, non-image items. This is
      // best-effort — we don't surface an error if the API is unavailable.
      const textItems = selectedItems.filter(
        (it) => !it.is_sensitive && !isImageType(it.content_type)
      );
      if (textItems.length > 1 && typeof navigator?.clipboard?.writeText === "function") {
        const concatenated = textItems.map((it) => it.preview).join("\n");
        try {
          await navigator.clipboard.writeText(concatenated);
        } catch {
          // Clipboard API unavailable or permission denied — daemon copy above already succeeded.
        }
      }

      clearSelection();
      void load(true);
      // Fire sound / notification on successful bulk copy — same gates as row-click.
      if (playSoundOnCopy) {
        void playCopySound();
      }
      if (notifyOnCopy) {
        // Use content_type + preview from the first selected item for the banner.
        const firstItem = selectedItems[0];
        void showCopyNotification(
          firstItem?.content_type ?? "",
          firstItem?.preview ?? ""
        );
      }
      showToast(`Copied ${selectedItems.length} item${selectedItems.length === 1 ? "" : "s"}`, "success");
    } finally {
      // Always release the busy flag — even if clearSelection/load throws,
      // so the bulk action bar is never permanently disabled (V-13).
      setBulkBusy(false);
    }
  }, [bulkBusy, multiSelectedIds, filtered, clearSelection, load, showToast, playSoundOnCopy, notifyOnCopy]);


  // Destructive database reset — the recovery escape hatch when the daemon is
  // degraded (DB cannot be decrypted). Erases all local history and recreates a
  // fresh empty database; the daemon recovers in-place. On success we re-fetch
  // history so the now-healthy (empty) view replaces the error screen; on
  // failure we keep the error visible and surface the real message (loud).
  const handleResetConfirmed = useCallback(async () => {
    setResetting(true);
    try {
      await resetDatabase();
      setResetConfirm(false);
      setDegraded(false);
      setErrorDetail(null);
      setSelectedId(null);
      setItems([]);
      clearImageCache(); // the items are gone; drop their cached thumbnails too
      sigRef.current = "";
      showToast("Database reset — local history erased", "success");
      await load(false);
    } catch (err) {
      // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
      console.error("[HistoryView] database reset error:", err);
      const msg = friendlyIpcError(err);
      setErrorDetail(`Reset failed: ${msg}`);
      showToast(`Reset failed: ${msg}`, "error");
    } finally {
      setResetting(false);
    }
  }, [load, showToast, setDegraded, setErrorDetail, setItems, sigRef]);

  // kayk: Clear all clipboard history — calls delete_all and reloads.
  // Wrapped behind ConfirmModal so it can't be triggered by a misclick.
  const handleClearAllConfirmed = useCallback(async () => {
    setClearAllBusy(true);
    try {
      const result = await api.deleteAll();
      setClearAllConfirmOpen(false);
      setItems([]);
      clearImageCache();
      sigRef.current = "";
      showToast(`Cleared ${result.deleted} item${result.deleted === 1 ? "" : "s"}`, "success");
      void load(true);
    } catch (err) {
      // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
      console.error("[HistoryView] clear-all error:", err);
      const msg = friendlyIpcError(err);
      showToast(`Clear failed: ${msg}`, "error");
      setClearAllConfirmOpen(false);
    } finally {
      setClearAllBusy(false);
    }
  }, [load, showToast, setItems, sigRef]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const actions = (
    <>
      {/* D2: hidden file input + attach button */}
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => void handleFileInputChange(e)}
        aria-label="Add file to clipboard history"
        tabIndex={-1}
      />
      {/* kp6f: borderRadius uses var(--skin-r-ctl) inline instead of rounded-ide class */}
      <button
        type="button"
        title="Add file to clipboard history"
        aria-label="Add file"
        onClick={() => fileInputRef.current?.click()}
        className="flex h-7 w-7 items-center justify-center border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text"
        style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
      >
        {/* Paperclip / attach icon */}
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M13.5 7.5 7 14a4.243 4.243 0 0 1-6-6l7-7a2.828 2.828 0 1 1 4 4L5.5 12A1.414 1.414 0 0 1 3.5 10L9 4.5" />
        </svg>
      </button>
      {/* Device filter dropdown — only shown when more than one device is present.
          kp6f: borderRadius via var(--skin-r-ctl) inline instead of rounded-ide. */}
      {knownDeviceIds.length > 1 && (
        <select
          value={deviceFilter}
          onChange={(e) => setDeviceFilter(e.target.value)}
          className="h-7 border border-ide-border bg-ide-elevated px-1.5 text-[11px] text-ide-text hover:bg-ide-hover cursor-pointer"
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
          aria-label="Filter by device"
          title="Filter by origin device"
        >
          <option value="all">All devices</option>
          {knownDeviceIds.map((id) => (
            <option key={id} value={id}>
              {deviceOptionLabel(id)}
            </option>
          ))}
        </select>
      )}

      {/* Sort-mode toggle — only shown when multiple devices are present */}
      {knownDeviceIds.length > 1 && (
        <button
          type="button"
          title={sortMode === "recency" ? "Sort by device" : "Sort by recency"}
          aria-label={sortMode === "recency" ? "Sort by device" : "Sort by recency"}
          onClick={toggleSortMode}
          className={[
            // kp6f: removed rounded-ide; borderRadius applied via inline style
            "flex h-7 items-center gap-1 border px-2 text-[11px]",
            sortMode === "device"
              ? "border-ide-accent/60 bg-ide-accent/10 text-ide-accent"
              : "border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text",
          ].join(" ")}
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
        >
          {/* Simple sort icon — two lines of different widths */}
          <svg viewBox="0 0 14 12" width="12" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
            <line x1="1" y1="2" x2="13" y2="2" />
            <line x1="1" y1="6" x2="9" y2="6" />
            <line x1="1" y1="10" x2="5" y2="10" />
          </svg>
          {sortMode === "device" ? "By device" : "By time"}
        </button>
      )}

      {/* Total-count badge — shows the full DB count from the daemon, not just
          the loaded slice. Hidden until the first page resolves (totalCount null). */}
      {totalCount !== null && (
        <span
          data-testid="history-total-badge"
          className="text-[11px] text-ide-faint tabular-nums"
          title="Total items in clipboard history"
        >
          {totalCount} {totalCount === 1 ? "item" : "items"}
        </span>
      )}
      {/* kayk: Clear all — destructive action hidden behind a ConfirmModal; only
          shown when there are items to delete (totalCount > 0) so the button
          doesn't appear on an already-empty history. */}
      {totalCount !== null && totalCount > 0 && (
        <button
          type="button"
          title="Clear all clipboard history"
          aria-label="Clear all"
          disabled={clearAllBusy}
          onClick={() => setClearAllConfirmOpen(true)}
          className="flex h-7 items-center gap-1 border border-ide-danger/50 bg-ide-elevated px-2 text-[11px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
        >
          {/* Trash icon */}
          <svg viewBox="0 0 14 14" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polyline points="1 3.5 2.5 3.5 13 3.5" />
            <path d="M11.5 3.5l-.75 8.5h-7.5L2.5 3.5" />
            <path d="M5 3.5V2a.5.5 0 0 1 .5-.5h3a.5.5 0 0 1 .5.5v1.5" />
          </svg>
          Clear all
        </button>
      )}
      {/* Search bar: premium focus ring — accent glow + smooth transition per styleguide §searchbar. */}
      <input
        ref={searchRef}
        type="search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Filter…"
        className={[
          // kp6f: removed rounded-ide; borderRadius via inline style
          "h-7 w-44 px-2 text-[12px]",
          "border border-ide-border bg-ide-elevated/80 text-ide-text placeholder:text-ide-faint",
          "transition-[border-color,box-shadow] duration-200 ease-out",
          "focus:outline-none focus:border-ide-accent/60",
          "focus:[box-shadow:0_0_0_3px_color-mix(in_srgb,var(--ide-accent)_18%,transparent)]",
        ].join(" ")}
        style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
      />
    </>
  );

  let body: React.ReactNode;

  if (loadState === "loading") {
    // CopyPaste-bdac.92: replaced plain text with an animated spinner consistent
    // with DevicesView (animate-spin border ring, motion-reduce-safe). No shared
    // Spinner component exists; inline pattern mirrors DevicesView exactly.
    body = (
      <div className="flex h-full items-center justify-center gap-2 text-[13px] text-ide-dim">
        <span
          className="inline-block h-4 w-4 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent"
          aria-hidden="true"
        />
        Loading…
      </div>
    );
  } else if (loadState === "offline") {
    body = (
      // reveal-up: glass-card entrance animation per styleguide §empty-state.
      <EmptyState
        className="h-full reveal-up"
        icon={
          // network-rings: discovery ring pulse on the icon — matches §empty-icon ::before/::after.
          <span className="network-rings inline-flex" style={{ borderRadius: 12 }}>
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </span>
        }
        title="Clipboard service offline"
        body="The background service is not running."
        action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void load()} /></div>}
      />
    );
  } else if (loadState === "not_ready") {
    // bdac.6: mirrors DevicesView not_ready branch — friendly "Starting up…"
    // instead of the error/degraded state. No errorDetail is ever set here.
    body = (
      <EmptyState
        className="h-full reveal-up"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
        }
        title="Starting up…"
        body="The clipboard service is initialising. History will appear in a moment."
      />
    );
  } else if (loadState === "error") {
    body = (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center"
        role="alert"
        aria-live="assertive"
      >
        <div className="text-[13px] font-medium text-ide-danger">
          {degraded ? "Clipboard database can't be opened" : "Failed to load history."}
        </div>
        {errorDetail && (
          <div className="max-w-md text-[12px] text-ide-dim break-words">{errorDetail}</div>
        )}
        {degraded && (
          <>
            <div className="max-w-md text-[12px] text-ide-dim">
              The local database could not be decrypted (its key no longer matches).
              You can reset it to recover — this permanently erases this device's
              clipboard history.
            </div>
            {/* 5j9x: replaced misclick-prone inline Yes/No with a ConfirmModal.
                Clicking the button opens the modal; the modal calls handleResetConfirmed
                only after the user explicitly confirms. */}
            {/* CopyPaste-5917.39: replaced rounded-ide with skin-token radius so
                Vapor (12px) and Quiet (7px) skins render the correct shape. */}
            <button
              onClick={() => setResetConfirm(true)}
              className="border border-ide-danger/60 bg-ide-elevated px-3 py-1.5 text-[12px] font-medium text-ide-danger hover:bg-ide-hover"
              style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
            >
              Reset database (erases local history)
            </button>
          </>
        )}
        {!degraded && (
          <RestartDaemonButton label="Restart background service" onRestarted={() => void load()} />
        )}
      </div>
    );
  } else if (filtered.length === 0 && items.length === 0) {
    body = (
      // reveal-up: glass-card entrance animation per styleguide §empty-state.
      <EmptyState
        className="h-full reveal-up"
        icon={
          // network-rings: discovery ring pulse on the icon — matches §empty-icon ::before/::after.
          <span className="network-rings inline-flex" style={{ borderRadius: 12 }}>
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
              <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
            </svg>
          </span>
        }
        title={isPrivateMode ? "Private mode is on" : "Nothing copied yet"}
        body={isPrivateMode ? "Clipboard is not recorded while private mode is active." : "Copy something and it will appear here."}
      />
    );
  } else if (filtered.length === 0) {
    body = (
      // reveal-up entrance; no network-rings on the search-empty state (different semantic).
      <EmptyState
        className="h-full reveal-up"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="7" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        }
        title={`No results for "${search}"`}
        body="Try a different search term."
      />
    );
  } else {
    body = (
      // Outer wrapper so the bulk bar and list share the same flex column.
      <div className="flex h-full flex-col overflow-hidden">
        {/* Bulk action bar — rendered above the list when items are selected */}
        {multiSelectedIds.size > 0 && (
          <BulkActionBar
            count={multiSelectedIds.size}
            allSelected={allSelected}
            onSelectAll={selectAll}
            onClearSelection={clearSelection}
            onBulkCopy={() => void handleBulkCopy()}
            onBulkPin={() => void handleBulkPin(true)}
            onBulkUnpin={() => void handleBulkPin(false)}
            onBulkDelete={() => setBulkDeleteConfirmOpen(true)}
            isBusy={bulkBusy}
          />
        )}
        {/* W-C3 / 10lk: Inset wrapper — adds padding around the VirtualList for inset rows.
            Driven by rowTreatment token (not skin name) so a future skin with rowTreatment="inset"
            gets the wrapper automatically. Per-row vertical gap is applied as marginBottom on each
            row (o2o9 fix: flex gap on this wrapper is a no-op because VirtualList rows are
            absolutely positioned). Classic/quiet (card/line) use no wrapper padding. */}
        <div
          className={SKINS[skin ?? "classic"].rowTreatment === "inset" ? "skin-list-vapor flex-1 overflow-hidden" : "flex-1 overflow-hidden"}
          style={SKINS[skin ?? "classic"].rowTreatment === "inset"
            ? { padding: "var(--skin-row-gap, 0px)" }
            : {}}
        >
        {/* SCRH-9: Show a subtle hint when the display-limit pref caps the visible list so
            the user isn't confused about why fewer items appear than the total-count badge
            shows. The sentinel value 100000 is used for "Unlimited" in settings. */}
        {(() => {
          const limit = historyDisplayLimit ?? 1000;
          const isTruncated = limit < 100000 && filtered.length > limit;
          if (!isTruncated) return null;
          return (
            <div
              className="shrink-0 border-b border-ide-divider/40 px-3 py-1 text-[11px] text-ide-faint text-center"
              aria-live="polite"
              data-testid="history-display-limit-hint"
            >
              Showing first {limit.toLocaleString()} of {filtered.length.toLocaleString()} results
              {" — "}
              <span className="text-ide-dim">adjust the display limit in Settings › Storage</span>
            </div>
          );
        })()}
        <VirtualList
          // Cap the rendered list to the persisted display-limit preference.
          // Sentinel 100000 means "Unlimited" (effectively uncapped for any realistic history).
          // The daemon may hold more items on disk; this is a UI rendering cap only.
          items={filtered.slice(0, historyDisplayLimit ?? 1000)}
          previewSize={previewSize}
          imageMaxHeight={imageMaxHeight}
          density={density}
          glideStyle={glideStyle}
          listRef={listRef}
          onKeyDown={(e) => void handleKeyDown(e)}
          // Only trigger load-more when not filtering: filtered view operates
          // over the already-loaded set, so near-bottom doesn't mean "more data
          // to fetch" — it just means the user has reached the end of the match.
          onNearBottom={search.trim() === "" ? handleNearBottom : undefined}
          activeDescendantId={selectedId ? `clip-${selectedId}` : null}
          renderRow={(entry, visibleIndex) => (
            <HistoryRow
              key={entry.id}
              entry={entry}
              selected={entry.id === selectedId}
              multiSelected={multiSelectedIds.has(entry.id)}
              selectionMode={selectionMode}
              previewLines={previewLinesApp}
              previewSize={previewSize}
              imageMaxHeight={imageMaxHeight}
              density={density}
              staggerIndex={visibleIndex}
              applyStagger={staggerActiveRef.current && visibleIndex < 10}
              maskSensitive={maskSensitive}
              showSensitiveWarnings={showSensitiveWarnings ?? true}
              ownDeviceId={ownDeviceId}
              skin={skin ?? "classic"}
              onSelect={() => {
                isKeyboardNavRef.current = false;
                setSelectedId(entry.id);
                listRef.current?.focus();
              }}
              onToggleMultiSelect={(e) => {
                e.stopPropagation();
                toggleMultiSelect(entry.id);
              }}
              onCopy={() => void handleCopy(entry.id)}
              onPin={() => void handlePin(entry.id, entry.pinned)}
              onDelete={() => handleDelete(entry.id, entry.preview)}
              onPreview={() => setPreviewEntry(entry)}
              onMouseEnter={() => {
                isKeyboardNavRef.current = false;
              }}
              dragHandleProps={
                entry.pinned
                  ? {
                      dragging: dragId === entry.id,
                      dropIndicator:
                        dropTarget?.id === entry.id ? dropTarget.position : null,
                      onDragStart: (e: React.DragEvent) => {
                        e.dataTransfer.effectAllowed = "move";
                        e.dataTransfer.setData("text/plain", entry.id);
                        setDragId(entry.id);
                      },
                      onDragOver: (e: React.DragEvent) => {
                        // Only accept drops from within the pinned section.
                        if (dragId === null) return;
                        e.preventDefault();
                        e.dataTransfer.dropEffect = "move";
                        // Determine above / below by cursor position within row.
                        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        const mid = rect.top + rect.height / 2;
                        const position: "above" | "below" = e.clientY < mid ? "above" : "below";
                        setDropTarget({ id: entry.id, position });
                      },
                      onDragLeave: () => {
                        setDropTarget((prev) =>
                          prev?.id === entry.id ? null : prev
                        );
                      },
                      onDrop: (e: React.DragEvent) => {
                        e.preventDefault();
                        const sourceId = e.dataTransfer.getData("text/plain");
                        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        const mid = rect.top + rect.height / 2;
                        const position: "above" | "below" = e.clientY < mid ? "above" : "below";
                        setDragId(null);
                        setDropTarget(null);
                        if (sourceId && sourceId !== entry.id) {
                          void handleReorderDrop(sourceId, entry.id, position);
                        }
                      },
                      onDragEnd: () => {
                        setDragId(null);
                        setDropTarget(null);
                      },
                    }
                  : undefined
              }
            />
          )}
        />
        </div>
      </div>
    );
  }

  return (
    <ViewShell title="History" actions={actions}>
      {/*
        D3 drop-zone overlay: shown while OS files are hovering over the window.
        The overlay sits above the content (z-10) and shows a dashed border +
        label so the user knows dropping is accepted. Pointer-events are none
        on the inner label so the Tauri drag event fires on the webview, not
        on a React element.
      */}
      <div className="relative h-full">
        {fileDragOver && (
          <div
            aria-hidden="true"
            // CopyPaste-5917.39: replaced rounded-ide with skin-token radius (card).
            className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-ide-accent bg-ide-accent/5"
            style={{ borderRadius: "var(--skin-r-card, 12px)" }}
          >
            <div className="flex flex-col items-center gap-2 text-ide-accent">
              <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="17 8 12 3 7 8" />
                <line x1="12" y1="3" x2="12" y2="15" />
              </svg>
              <span className="text-[13px] font-medium">Drop to add to clipboard</span>
            </div>
          </div>
        )}
        {body}
      </div>
      {/* F11: Undo-delete toast — shown while a deferred delete is pending */}
      {undoPending !== null && (
        <div
          // surface-glass-strong = same floating frosted-glass material as Toast,
          // theme-aware (replaces the hardcoded dark-only rgba fill + blur).
          // SCRH-12: z-40 keeps the undo toast BELOW the DetailsModal (z-50) so
          // an open modal is never occluded by a transient notification.
          // (Previously z-[9999] rendered this toast on top of everything.)
          // CopyPaste-bdac.58: padding now via Tailwind classes (pl-2.5 pr-3.5 py-1.5)
          // instead of hardcoded inline "6px 14px 6px 10px" so density tokens apply.
          className="surface-glass-strong toast-enter fixed bottom-3 left-1/2 z-40 pointer-events-auto flex items-center gap-2.5 whitespace-nowrap pl-2.5 pr-3.5 py-1.5"
          role="status"
          aria-live="polite"
          style={{
            transform: "translateX(-50%)",
            // CopyPaste-bdac.54: fallback corrected to 12px (Classic skin canonical value).
            borderRadius: "var(--skin-r-card, 12px)",
          }}
        >
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              flexShrink: 0,
              // CopyPaste-bdac.30: fallback matches dark-mode --ide-danger token (#E05C5C).
              background: "var(--ide-danger, #e05c5c)",
            }}
          />
          <span className="text-[12px] text-ide-text">
            Deleted &ldquo;
            {undoPending.preview.length > 40
              ? `${undoPending.preview.slice(0, 40)}…`
              : undoPending.preview}
            &rdquo;
          </span>
          <button
            onClick={handleUndo}
            className="text-[12px] font-semibold text-ide-accent"
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: 0,
              flexShrink: 0,
            }}
          >
            Undo
          </button>
        </div>
      )}
      {/* M10: Details modal */}
      {previewEntry !== null && (
        <DetailsModal entry={previewEntry} maskSensitive={maskSensitive} showSensitiveWarnings={showSensitiveWarnings ?? true} onClose={() => setPreviewEntry(null)} />
      )}
      {/* fjvz: bulk-delete confirmation modal — requires explicit user consent
          before mass-deleting selected items. Undo is not available for bulk
          delete (too many items to hold optimistically), so we confirm first. */}
      <ConfirmModal
        open={bulkDeleteConfirmOpen}
        title={`Delete ${multiSelectedIds.size} item${multiSelectedIds.size === 1 ? "" : "s"}?`}
        body="This will permanently remove the selected clipboard items. This action cannot be undone."
        confirmLabel="Delete"
        busy={bulkBusy}
        onConfirm={() => {
          setBulkDeleteConfirmOpen(false);
          void handleBulkDelete();
        }}
        onCancel={() => setBulkDeleteConfirmOpen(false)}
      />
      {/* 5j9x: Reset database — replaces the inline Yes/No confirm with a proper modal
          so accidental clicks in the degraded error state don't wipe the database. */}
      <ConfirmModal
        open={resetConfirm}
        title="Reset clipboard database?"
        body="This will permanently erase all clipboard history on this device and recreate a fresh database. This cannot be undone."
        confirmLabel="Erase and reset"
        busy={resetting}
        onConfirm={() => void handleResetConfirmed()}
        onCancel={() => setResetConfirm(false)}
      />
      {/* kayk: Clear all — destructive delete_all behind a confirm modal, matching
          Android and CLI behaviour. The modal prevents accidental mass-deletion. */}
      <ConfirmModal
        open={clearAllConfirmOpen}
        title="Clear all clipboard history?"
        body="This will permanently delete all clipboard items on this device. This cannot be undone."
        confirmLabel="Clear all"
        busy={clearAllBusy}
        onConfirm={() => void handleClearAllConfirmed()}
        onCancel={() => setClearAllConfirmOpen(false)}
      />
    </ViewShell>
  );
}

// CopyPaste-5917.102: HistoryView wraps HistoryViewInner in ToastProvider so
// useToast() calls inside the inner component have a provider in the tree.
// This self-contained approach avoids touching App.tsx while removing the
// local Toast duplicate.
export function HistoryView() {
  return (
    <ToastProvider>
      <HistoryViewInner />
    </ToastProvider>
  );
}
