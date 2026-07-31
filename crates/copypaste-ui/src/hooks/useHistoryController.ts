import { useCallback, useMemo, useState } from "react";
import { useDebounceValue } from "usehooks-ts";

import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { historyOf, useClearHistory, useHistory, useStatus } from "@/hooks/useHistory";
import { type ErrorKind, classifyError } from "@/lib/errors";
import type { Item } from "@/lib/ipc";
import { SEARCH_DEBOUNCE_MS } from "@/lib/layout";
import {
  DEFAULT_VIEW,
  type ViewOptions,
  applyView,
  isDefaultView,
} from "@/lib/view";
import { useUi } from "@/store/ui";

export interface HistoryController {
  readonly rawQuery: string;
  readonly setRawQuery: (query: string) => void;
  readonly query: string;
  readonly view: ViewOptions;
  readonly setView: (view: ViewOptions) => void;
  readonly items: readonly Item[];
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

/** The screen's only route to `useHistory`, so a change to how pages are
 *  fetched lands here and nowhere else. */
export function useHistoryController(pushLive: boolean): HistoryController {
  const rawQuery = useUi((s) => s.query);
  const setRawQuery = useUi((s) => s.setQuery);

  // §5.3: the FTS query is debounced 250ms. `usehooks-ts` owns the timer —
  // there is no reason for this repository to carry a fifth debounce.
  const [query] = useDebounceValue(rawQuery, SEARCH_DEBOUNCE_MS);

  const [view, setView] = useState<ViewOptions>(DEFAULT_VIEW);
  const history = useHistory(query, pushLive);
  const status = useStatus();
  const clear = useClearHistory();
  const { pending, remove } = useDeferredDelete();

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

  const loadMore = useCallback(() => {
    if (history.hasNextPage && !history.isFetchingNextPage) {
      void history.fetchNextPage();
    }
  }, [history]);

  const retry = useCallback(() => {
    void history.refetch();
  }, [history]);

  const searching = query.length > 0;

  return {
    rawQuery,
    setRawQuery,
    query,
    view,
    setView,
    items,
    skipped: page.skipped,
    total: status.data?.item_count,
    loading: history.isPending,
    errorKind: history.error ? classifyError(history.error) : null,
    searching,
    filtered: searching || !isDefaultView(view),
    hasMore: history.hasNextPage,
    loadingMore: history.isFetchingNextPage,
    loadMore,
    retry,
    remove,
    clearAll: clear.mutate,
  };
}
