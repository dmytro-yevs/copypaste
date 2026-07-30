/**
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing else to show. A background poll
 * that fails while 200 rows are on screen must not throw those rows away — the
 * banner and the status chip say the service went away, and the rows stay
 * readable.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDebounceValue } from "usehooks-ts";
import { CircleAlert, Inbox, Search, ShieldAlert } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { buttonVariants } from "@/components/ui/button";
import { CaptureStatus } from "@/components/capture/CaptureStatus";
import { EmptyState } from "@/components/EmptyState";
import { BulkBar } from "@/components/history/BulkBar";
import { HistoryList } from "@/components/history/HistoryList";
import { QuickHint } from "@/components/history/QuickHint";
import { SearchBar } from "@/components/history/SearchBar";
import { SkippedNotice } from "@/components/history/SkippedNotice";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import {
  historyOf,
  useBulkDelete,
  useBulkPin,
  useClearHistory,
  useCopy,
  useHistory,
  usePin,
  useStatus,
} from "@/hooks/useHistory";
import { useReveal } from "@/hooks/useReveal";
import { useSelection } from "@/hooks/useSelection";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { type ErrorKind, classifyError, friendlyError } from "@/lib/errors";
import { hideWindow } from "@/lib/ipc";
import type { Item } from "@/lib/ipc";
import { SEARCH_DEBOUNCE_MS } from "@/lib/layout";
import { DEFAULT_VIEW, type ViewOptions, applyView, isDefaultView } from "@/lib/view";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

interface HistoryViewProps {
  /** From `usePush` at the app root: slows the poll without stopping it. */
  pushLive?: boolean;
}

export function HistoryView({ pushLive = false }: HistoryViewProps) {
  const { t } = useTranslation();
  const rawQuery = useUi((s) => s.query);
  const setRawQuery = useUi((s) => s.setQuery);
  const activeId = useUi((s) => s.activeId);
  const setActiveId = useUi((s) => s.setActiveId);

  const previewLines = usePrefs((s) => s.previewLines);
  const warnBeforeReveal = usePrefs((s) => s.warnBeforeReveal);

  // §5.3: the FTS query is debounced 250ms. `usehooks-ts` owns the timer —
  // there is no reason for this repository to carry a fifth debounce.
  const [query] = useDebounceValue(rawQuery, SEARCH_DEBOUNCE_MS);

  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const [view, setView] = useState<ViewOptions>(DEFAULT_VIEW);
  const history = useHistory(query, pushLive);
  const status = useStatus();
  const copy = useCopy();
  const pin = usePin();
  const clearAll = useClearHistory();
  const bulkPin = useBulkPin();
  const bulkDelete = useBulkDelete();
  const { pending, remove } = useDeferredDelete();
  const reveal = useReveal();

  const [confirmReveal, setConfirmReveal] = useState<Item | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [confirmBulkDelete, setConfirmBulkDelete] = useState(false);

  /**
   * INV-2: when nothing is pending and the view is the default this returns
   * the query's own array, so an idle poll that fetched byte-identical data
   * produces the identical reference React Query's structural sharing handed
   * us — no re-render, and the scroll anchor is never disturbed.
   */
  const page = historyOf(history.data);
  const items = useMemo(() => {
    const shown =
      pending.size === 0
        ? page.items
        : page.items.filter((item) => !pending.has(item.id));
    return applyView(shown, view);
  }, [page.items, pending, view]);

  const selection = useSelection(items);

  const errorKind: ErrorKind | null = history.error
    ? classifyError(history.error)
    : null;

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

  const loadMore = useCallback(() => {
    if (history.hasNextPage && !history.isFetchingNextPage) {
      void history.fetchNextPage();
    }
  }, [history]);

  const onReveal = useCallback(
    (item: Item) => {
      // n9gp / PG-34: the warning is a preference, default on, and the reveal
      // itself is one click away either way.
      if (warnBeforeReveal) setConfirmReveal(item);
      else void reveal.reveal(item.id);
    },
    [reveal, warnBeforeReveal],
  );

  /**
   * ⌘1–⌘9: copy, then dismiss.
   *
   * Copy first and hide afterwards (INV-26). Hiding first swallows the failure
   * — the toast would render into a window nobody can see — and leaves the
   * user pressing ⌘V for something that never reached the clipboard.
   */
  const quickCopy = useCallback(
    (item: Item) => {
      copy.mutate(item, {
        onSuccess: () => {
          // INV-25: through the backend, never `window.hide()` from here. The
          // app is an Accessory on macOS, so this hands activation back to
          // whatever the user was in — which is where they press ⌘V.
          void hideWindow().catch(() => {
            // A build with no window to hide (the browser, a test) is not a
            // failed copy. The copy already happened.
          });
        },
      });
    },
    [copy],
  );

  const searching = query.length > 0;
  const filtered = searching || !isDefaultView(view);
  const total = status.data?.item_count;
  const busy = bulkPin.isPending || bulkDelete.isPending;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <SearchBar
        value={rawQuery}
        onChange={setRawQuery}
        onEnterList={() => listRef.current?.focus()}
        inputRef={searchRef}
        filtered={filtered}
        visible={items.length}
        total={total}
        view={view}
        onViewChange={setView}
        selecting={selection.selecting}
        onToggleSelecting={() =>
          selection.selecting ? selection.end() : selection.begin()
        }
        onClearAll={
          items.length > 0 && !filtered ? () => setConfirmClear(true) : undefined
        }
      />

      {selection.selecting && (
        <BulkBar
          count={selection.items.length}
          allPinned={selection.allPinned}
          busy={busy}
          onTogglePin={() =>
            bulkPin.mutate(
              { items: selection.items, pinned: !selection.allPinned },
              { onSettled: () => selection.end() },
            )
          }
          onDelete={() => setConfirmBulkDelete(true)}
          onCancel={selection.end}
        />
      )}

      {/* §5 rule 1: which rung is live is visible wherever the history is,
          not buried in a diagnostics screen. */}
      <CaptureStatus />

      <SkippedNotice count={page.skipped} />

      {history.isPending ? (
        <EmptyState
          busy
          title={t("history.empty.loading.title")}
          body={t("history.empty.loading.body")}
        />
      ) : items.length > 0 ? (
        <HistoryList
          items={items}
          activeId={activeId}
          onActiveIdChange={setActiveId}
          revealedId={reveal.revealedId}
          revealedContent={reveal.revealedContent}
          revealPendingId={reveal.pendingId}
          previewLines={previewLines}
          searching={searching}
          selection={selection}
          hasMore={history.hasNextPage}
          loadingMore={history.isFetchingNextPage}
          onReveal={onReveal}
          onHide={reveal.hide}
          onCopy={copy.mutate}
          onQuickCopy={quickCopy}
          onTogglePin={pin.mutate}
          onDelete={remove}
          onLoadMore={loadMore}
          listRef={listRef}
        />
      ) : errorKind === "offline" ? (
        <ServiceOffline />
      ) : errorKind === "not_ready" ? (
        <EmptyState
          busy
          title={t("history.empty.starting.title")}
          body={friendlyError("not_ready")}
        />
      ) : errorKind !== null ? (
        <EmptyState
          icon={CircleAlert}
          title={t("history.empty.failed.title")}
          body={friendlyError(errorKind)}
          action={{
            label: t("common.tryAgain"),
            onClick: () => void history.refetch(),
          }}
        />
      ) : filtered ? (
        <EmptyState
          icon={Search}
          title={
            searching
              ? t("history.empty.noResults", { query })
              : t("history.empty.noMatch")
          }
          body={t("history.empty.filteredBody")}
          action={
            history.hasNextPage
              ? { label: t("history.empty.loadMore"), onClick: loadMore }
              : undefined
          }
        />
      ) : (
        <EmptyState
          icon={Inbox}
          title={t("history.empty.none.title")}
          body={t("history.empty.none.body")}
        />
      )}

      <QuickHint searching={searching} />

      {/* A refused reveal is a state, not a failure — see useReveal. It is
          rendered where the row is, dismissible, and never carries a raw
          error. */}
      {reveal.error && (
        <div
          role="alert"
          className="flex shrink-0 items-start gap-s-2 border-t border-warn/20 bg-warn/15 px-s-3 py-s-2 text-xs text-warn-strong"
        >
          <ShieldAlert size={14} aria-hidden="true" className="mt-px shrink-0" />
          <span className="min-w-0 flex-1">{reveal.error}</span>
          <button
            type="button"
            onClick={reveal.hide}
            className="shrink-0 underline underline-offset-2 outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
          >
            {t("common.dismiss")}
          </button>
        </div>
      )}

      {/* One confirm dialog at a time (INV-18): each is driven by its own piece
          of state and opening any of them closes the row's own handlers. */}
      <AlertDialog
        open={confirmReveal !== null}
        onOpenChange={(open) => !open && setConfirmReveal(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("history.reveal.confirm.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.reveal.confirm.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (confirmReveal) void reveal.reveal(confirmReveal.id);
                setConfirmReveal(null);
              }}
            >
              {t("history.reveal.confirm.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Bulk delete has no undo window, unlike the single-row delete
          (§3.1.9), so this dialog is the only gate in front of it. */}
      <AlertDialog open={confirmBulkDelete} onOpenChange={setConfirmBulkDelete}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("history.bulkDelete.title", { count: selection.items.length })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.bulkDelete.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                bulkDelete.mutate(selection.items, {
                  onSettled: () => selection.end(),
                });
                setConfirmBulkDelete(false);
              }}
            >
              {t("history.bulkDelete.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={confirmClear} onOpenChange={setConfirmClear}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("history.clear.title")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("history.clear.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                clearAll.mutate();
                setConfirmClear(false);
              }}
            >
              {t("history.clear.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
