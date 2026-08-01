import { useCallback, useEffect, useMemo, useState } from "react";
import { useDebounceValue } from "usehooks-ts";

import { type OriginDevice, originsOf } from "@/components/history/origin";
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import {
  historyOf,
  useClearHistory,
  useHistory,
  useHistorySearch,
  useStatus,
} from "@/hooks/useHistory";
import { type ErrorKind, classifyError } from "@/lib/errors";
import type { Item } from "@/lib/ipc";
import { SEARCH_DEBOUNCE_MS } from "@/lib/layout";
import {
  DEFAULT_VIEW,
  type ViewOptions,
  applyView,
  fuzzyItems,
  isFilteringView,
  mergeSearchResults,
} from "@/lib/view";
import { UNLIMITED_HISTORY_DISPLAY, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const EMPTY_ITEMS: readonly Item[] = [];

export interface HistoryController {
  readonly rawQuery: string;
  readonly setRawQuery: (query: string) => void;
  readonly query: string;
  readonly view: ViewOptions;
  readonly setView: (view: ViewOptions) => void;
  readonly items: readonly Item[];
  readonly resultCount: number;
  readonly origins: readonly OriginDevice[];
  readonly groupedByDevice: boolean;
  readonly displayLimit: number | null;
  readonly skipped: number;
  readonly total: number | undefined;
  readonly loading: boolean;
  readonly errorKind: ErrorKind | null;
  readonly searching: boolean;
  readonly filtered: boolean;
  readonly hasMore: boolean;
  readonly loadingMore: boolean;
  readonly loadMore: () => void;
  readonly retry: () => void;
  readonly remove: (item: Item) => void;
  readonly clearAll: () => void;
}

export function useHistoryController(pushLive: boolean): HistoryController {
  const rawQuery = useUi((s) => s.query);
  const setRawQuery = useUi((s) => s.setQuery);
  const sortByDevice = usePrefs((s) => s.sortByDevice);
  const historyDisplayLimit = usePrefs((s) => s.historyDisplayLimit);
  const setPref = usePrefs((s) => s.set);

  const normalizedQuery = rawQuery.trim();

  // §5.3: the FTS query is debounced 250ms. `usehooks-ts` owns the timer —
  // there is no reason for this repository to carry a fifth debounce.
  const [serverQuery] = useDebounceValue(normalizedQuery, SEARCH_DEBOUNCE_MS);

  const [view, setLocalView] = useState<ViewOptions>(() => ({
    ...DEFAULT_VIEW,
    groupByDevice: sortByDevice,
  }));
  const history = useHistory("", pushLive);
  const serverSearch = useHistorySearch(serverQuery);
  const status = useStatus();
  const clear = useClearHistory();
  const { pending, remove } = useDeferredDelete();

  /** INV-2: with nothing pending and the default view this returns the query's
   *  own array, so an idle poll that fetched identical data produces the same
   *  reference — no re-render, and the scroll anchor is not disturbed. */
  const page = historyOf(history.data);
  const searching = normalizedQuery.length > 0;
  const serverItems =
    serverQuery === normalizedQuery
      ? (serverSearch.data?.items ?? EMPTY_ITEMS)
      : EMPTY_ITEMS;
  const origins = useMemo(
    () => originsOf([...page.items, ...serverItems]),
    [page.items, serverItems],
  );

  const setView = useCallback(
    (next: ViewOptions) => {
      setLocalView(next);
      if (next.groupByDevice !== sortByDevice) {
        setPref("sortByDevice", next.groupByDevice);
      }
    },
    [setPref, sortByDevice],
  );

  useEffect(() => {
    setLocalView((current) =>
      current.groupByDevice === sortByDevice
        ? current
        : { ...current, groupByDevice: sortByDevice },
    );
  }, [sortByDevice]);

  useEffect(() => {
    if (
      view.device !== DEFAULT_VIEW.device &&
      (origins.length <= 1 || !origins.some((origin) => origin.id === view.device))
    ) {
      setLocalView((current) => ({ ...current, device: DEFAULT_VIEW.device }));
    }
  }, [origins, view.device]);

  useEffect(() => {
    if (searching && history.hasNextPage && !history.isFetchingNextPage) {
      void history.fetchNextPage();
    }
  }, [
    history.fetchNextPage,
    history.hasNextPage,
    history.isFetchingNextPage,
    page.items.length,
    searching,
  ]);

  const result = useMemo(() => {
    const shown =
      pending.size === 0
        ? page.items
        : page.items.filter((item) => !pending.has(item.id));
    const searched = searching
      ? mergeSearchResults(
          fuzzyItems(shown, normalizedQuery),
          serverItems.filter((item) => !pending.has(item.id)),
        )
      : shown;
    const all = applyView(searched, view, searching);
    const capped =
      historyDisplayLimit === UNLIMITED_HISTORY_DISPLAY ||
      all.length <= historyDisplayLimit
        ? all
        : all.slice(0, historyDisplayLimit);
    return { all, capped };
  }, [
    historyDisplayLimit,
    normalizedQuery,
    page.items,
    pending,
    searching,
    serverItems,
    view,
  ]);

  const loadMore = useCallback(() => {
    if (!searching && history.hasNextPage && !history.isFetchingNextPage) {
      void history.fetchNextPage();
    }
  }, [history, searching]);

  const retry = useCallback(() => {
    void history.refetch();
  }, [history]);

  return {
    rawQuery,
    setRawQuery,
    query: normalizedQuery,
    view,
    setView,
    items: result.capped,
    resultCount: result.all.length,
    origins,
    groupedByDevice:
      view.groupByDevice && origins.length > 1 && !searching,
    displayLimit:
      result.capped.length < result.all.length ? historyDisplayLimit : null,
    skipped: page.skipped,
    total: status.data?.item_count,
    loading: history.isPending,
    errorKind: history.error ? classifyError(history.error) : null,
    searching,
    filtered: searching || isFilteringView(view),
    hasMore: !searching && history.hasNextPage,
    loadingMore: history.isFetchingNextPage,
    loadMore,
    retry,
    remove,
    clearAll: clear.mutate,
  };
}
