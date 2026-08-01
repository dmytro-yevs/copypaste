import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { CaptureStatus } from "@/components/capture/CaptureStatus";
import { BulkBar } from "@/components/history/BulkBar";
import { HistoryContentState } from "@/components/history/HistoryContentState";
import { HistoryDetail } from "@/components/history/HistoryDetail";
import { HistoryDialogs } from "@/components/history/HistoryDialogs";
import { RevealNotice } from "@/components/history/RevealNotice";
import { SearchBar } from "@/components/history/SearchBar";
import { SkippedNotice } from "@/components/history/SkippedNotice";
import { markedOrigins, originLabel } from "@/components/history/origin";
import { useCopy, usePin, useStatus } from "@/hooks/useHistory";
import { useHistoryController } from "@/hooks/useHistoryController";
import { useHistorySelection } from "@/hooks/useHistorySelection";
import { useReveal } from "@/hooks/useReveal";
import { hideWindow } from "@/lib/ipc";
import type { Item } from "@/lib/ipc";
import { previewOf } from "@/lib/format";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

interface HistoryViewProps {
  /** From `usePush` at the app root: slows the poll without stopping it. */
  pushLive?: boolean;
}

export function HistoryView({ pushLive = false }: HistoryViewProps) {
  const activeId = useUi((s) => s.activeId);
  const setActiveId = useUi((s) => s.setActiveId);
  const setView = useUi((s) => s.setView);
  const previewLines = usePrefs((s) => s.previewLines);

  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const history = useHistoryController(pushLive);
  const bulk = useHistorySelection(history.items);
  const reveal = useReveal();
  const copy = useCopy();
  const pin = usePin();
  const status = useStatus();

  const [confirmClear, setConfirmClear] = useState(false);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [bulkCopying, setBulkCopying] = useState(false);

  const selection = bulk.selection;
  const items = history.items;

  // Resolved from the id on every render, never held: a poll can replace the
  // array while the view is open, and an item deleted underneath the reader
  // closes it rather than leaving a copy of a row that no longer exists.
  const detail = useMemo(() => {
    if (detailId === null) return null;
    const item = items.find((candidate) => candidate.id === detailId);
    return item
      ? { item, origin: originLabel(item, markedOrigins(items)) }
      : null;
  }, [detailId, items]);

  const openDetail = useCallback(
    (item: Item) => {
      setActiveId(item.id);
      setDetailId(item.id);
    },
    [setActiveId],
  );

  // ⌘F / Ctrl+F focuses the field and selects what is in it (§3.1.4).
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  /** Copy first, hide afterwards (INV-26): hiding first swallows the failure —
   *  the toast renders into a window nobody can see — and leaves the user
   *  pressing ⌘V for something that never reached the clipboard. */
  const quickCopy = useCallback(
    (item: Item) => {
      copy.mutate(item, {
        onSuccess: () => {
          void hideWindow().catch(() => {
            // A build with no window to hide (the browser, a test) is not a
            // failed copy. The copy already happened.
          });
        },
      });
    },
    [copy],
  );

  const copySelected = useCallback(() => {
    const selected = selection.items;
    const first = selected[0];
    if (!first || bulkCopying) return;

    setBulkCopying(true);
    copy.mutate(first, {
      onSuccess: () => {
        const text = selected
          .filter(
            (item) =>
              !item.is_sensitive &&
              item.content !== null &&
              !item.content_type.toLowerCase().startsWith("image"),
          )
          .map((item) => previewOf(item.content!))
          .join("\n");
        const finish = () => {
          selection.end();
          setBulkCopying(false);
        };

        if (!text || typeof navigator.clipboard?.writeText !== "function") {
          finish();
          return;
        }

        void Promise.resolve()
          .then(() => navigator.clipboard.writeText(text))
          .catch(() => {
            // Best effort: the daemon copy has already succeeded (§3.1.9).
          })
          .finally(finish);
      },
      onError: () => setBulkCopying(false),
    });
  }, [bulkCopying, copy, selection]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <SearchBar
        value={history.rawQuery}
        onChange={history.setRawQuery}
        onEnterList={() => listRef.current?.focus()}
        inputRef={searchRef}
        filtered={history.filtered}
        visible={history.resultCount}
        total={history.total}
        view={history.view}
        onViewChange={history.setView}
        origins={history.origins}
        displayLimit={history.displayLimit}
        selecting={selection.selecting}
        onToggleSelecting={() =>
          selection.selecting ? selection.end() : selection.begin()
        }
        onClearAll={
          items.length > 0 && !history.filtered
            ? () => setConfirmClear(true)
            : undefined
        }
      />

      {selection.selecting && (
        <BulkBar
          count={selection.items.length}
          allPinned={selection.allPinned}
          busy={bulk.busy || bulkCopying}
          onCopy={copySelected}
          onTogglePin={bulk.togglePin}
          onDelete={bulk.requestDelete}
          onCancel={selection.end}
        />
      )}

      {/* §5 rule 1: which rung is live is visible wherever the history is,
          not buried in a diagnostics screen. */}
      <CaptureStatus />

      <SkippedNotice count={history.skipped} />

      <HistoryContentState
        loading={history.loading}
        errorKind={history.errorKind}
        searching={history.searching}
        filtered={history.filtered}
        privateMode={status.data?.private_mode === true}
        capturePaused={status.data?.capture_running === false}
        query={history.query}
        hasMore={history.hasMore}
        onLoadMore={history.loadMore}
        onRetry={history.retry}
        onOpenCapture={() => setView("capture")}
        list={{
          items,
          activeId,
          onActiveIdChange: setActiveId,
          revealedId: reveal.revealedId,
          revealedContent: reveal.revealedContent,
          revealPendingId: reveal.pendingId,
          previewLines,
          searching: history.searching,
          groupedByDevice: history.groupedByDevice,
          selection,
          hasMore: history.hasMore,
          loadingMore: history.loadingMore,
          onReveal: reveal.request,
          onCopy: copy.mutate,
          onQuickCopy: quickCopy,
          onTogglePin: pin.mutate,
          onDelete: history.remove,
          onOpen: openDetail,
          onLoadMore: history.loadMore,
          listRef,
        }}
      />

      <RevealNotice message={reveal.error} />

      <HistoryDetail
        item={detail?.item ?? null}
        origin={detail?.origin ?? null}
        revealedContent={
          detail && reveal.revealedId === detail.item.id
            ? reveal.revealedContent
            : null
        }
        revealPending={reveal.pendingId === detailId}
        onReveal={reveal.request}
        onHide={reveal.hide}
        onCopy={copy.mutate}
        onClose={() => setDetailId(null)}
        onReturnFocus={() => listRef.current?.focus()}
      />

      <HistoryDialogs
        reveal={{
          open: reveal.confirming,
          onCancel: reveal.cancel,
          onConfirm: reveal.confirm,
        }}
        bulkDelete={{
          open: bulk.confirmingDelete,
          count: selection.items.length,
          onCancel: bulk.cancelDelete,
          onConfirm: bulk.confirmDelete,
        }}
        clear={{
          open: confirmClear,
          onCancel: () => setConfirmClear(false),
          onConfirm: () => {
            history.clearAll();
            setConfirmClear(false);
          },
        }}
      />
    </div>
  );
}
