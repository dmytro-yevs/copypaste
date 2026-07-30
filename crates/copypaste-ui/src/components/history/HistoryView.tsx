/**
 * The History screen: search, the virtualised list, and the states it can be
 * in instead of a list.
 *
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
import { EmptyState } from "@/components/EmptyState";
import { HistoryList } from "@/components/history/HistoryList";
import { SearchBar } from "@/components/history/SearchBar";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import {
  useClearHistory,
  useCopy,
  useHistory,
  usePin,
  useStatus,
} from "@/hooks/useHistory";
import { useReveal } from "@/hooks/useReveal";
import { cn } from "@/lib/cn";
import { type ErrorKind, classifyError, friendlyError } from "@/lib/errors";
import type { Item } from "@/lib/ipc";
import { SEARCH_DEBOUNCE_MS } from "@/lib/layout";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const NO_ITEMS: readonly Item[] = [];

export function HistoryView() {
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

  const history = useHistory(query);
  const status = useStatus();
  const copy = useCopy();
  const pin = usePin();
  const clearAll = useClearHistory();
  const { pending, remove } = useDeferredDelete();
  const reveal = useReveal();

  const [confirmReveal, setConfirmReveal] = useState<Item | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  /**
   * INV-2: when nothing is pending this returns the query's own array, so an
   * idle poll that fetched byte-identical data produces the identical reference
   * React Query's structural sharing handed us — no re-render, and the scroll
   * anchor is never disturbed.
   */
  const items = useMemo(() => {
    const data = history.data ?? NO_ITEMS;
    return pending.size === 0
      ? data
      : data.filter((item) => !pending.has(item.id));
  }, [history.data, pending]);

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

  const filtered = query.length > 0;
  const total = status.data?.item_count;

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
        onClearAll={
          items.length > 0 && !filtered ? () => setConfirmClear(true) : undefined
        }
      />

      {history.isPending ? (
        <EmptyState busy title="Loading…" body="Fetching your clipboard history." />
      ) : items.length > 0 ? (
        <HistoryList
          items={items}
          activeId={activeId}
          onActiveIdChange={setActiveId}
          revealedId={reveal.revealedId}
          revealedContent={reveal.revealedContent}
          revealPendingId={reveal.pendingId}
          previewLines={previewLines}
          onReveal={onReveal}
          onHide={reveal.hide}
          onCopy={copy.mutate}
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
          title="Starting up…"
          body={friendlyError("not_ready")}
        />
      ) : errorKind !== null ? (
        <EmptyState
          icon={CircleAlert}
          title="Failed to load history"
          body={friendlyError(errorKind)}
          action={{ label: "Try again", onClick: () => void history.refetch() }}
        />
      ) : filtered ? (
        <EmptyState
          icon={Search}
          title={`No results for "${query}"`}
          body="Try a different search term. Sensitive items are never indexed, so they never appear in results."
        />
      ) : (
        <EmptyState
          icon={Inbox}
          title="Nothing copied yet"
          body="Copy something and it will appear here."
        />
      )}

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
            Dismiss
          </button>
        </div>
      )}

      {/* One confirm dialog at a time (INV-18): both are driven by their own
          piece of state and neither can be open while the other is, because
          opening either closes the row's own handlers. */}
      <AlertDialog
        open={confirmReveal !== null}
        onOpenChange={(open) => !open && setConfirmReveal(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reveal sensitive content?</AlertDialogTitle>
            <AlertDialogDescription>
              This item looks like a password, key or token. It will be shown for
              10 seconds, and hidden again as soon as this window loses focus.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (confirmReveal) void reveal.reveal(confirmReveal.id);
                setConfirmReveal(null);
              }}
            >
              Reveal
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={confirmClear} onOpenChange={setConfirmClear}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Clear all clipboard history?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes every unpinned item on this device.
              Pinned items are kept. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                clearAll.mutate();
                setConfirmClear(false);
              }}
            >
              Clear all
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
