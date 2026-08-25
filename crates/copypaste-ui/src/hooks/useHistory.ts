import { useMemo } from "react";
import { keepPreviousData, useInfiniteQuery, useQuery } from "@tanstack/react-query";

import {
  POLL_ACTIVE_MS,
  POLL_BACKOFF_MS,
  POLL_PUSH_BACKSTOP_MS,
} from "@/lib/scheduling";
import {
  HISTORY_PAGE_SIZE,
  HISTORY_SEARCH_LIMIT,
} from "@/lib/historyLimits";
import {
  HISTORY_HEAD_KEY,
  HISTORY_SEARCH_KEY,
  historyKey,
} from "@/hooks/historyRefresh";
import { type Item, type ItemPage, listItems, searchItems } from "@/lib/ipc";

export interface History {
  readonly items: readonly Item[];
  readonly skipped: number;
}

const EMPTY: History = { items: [], skipped: 0 };

/**
 * Never `false`. A subscription can die without either end noticing — a killed
 * daemon, a socket half-closed by sleep/wake — and a window that stopped
 * polling on one `live` event shows yesterday's clipboard until it is
 * restarted. The backstop bounds that to half a minute, at 2 requests a minute
 * against poll-only's 20.
 */
export function pollInterval(pushLive: boolean, failed: boolean): number {
  if (failed) return POLL_BACKOFF_MS;
  return pushLive ? POLL_PUSH_BACKSTOP_MS : POLL_ACTIVE_MS;
}

/**
 * De-duplicated by id: a cursor cannot hand the same row out twice, but a pin
 * moves a row between sections mid-walk, so two held pages can still name it
 * (INV-4). First occurrence wins, so the list does not jump under the scroll.
 * Skipped counts sum across pages; the last page's alone would shrink as the
 * user scrolls.
 */
function flatten(pages: readonly ItemPage[]): History {
  const seen = new Set<string>();
  const items: Item[] = [];
  let skipped = 0;
  for (const page of pages) {
    skipped += page.skipped_undecryptable;
    for (const item of page.items) {
      if (seen.has(item.id)) continue;
      seen.add(item.id);
      items.push(item);
    }
  }
  return { items, skipped };
}

function flattenPages(data: { pages: ItemPage[] }): History {
  return flatten(data.pages);
}

/**
 * The head re-reads the window the first cached page holds, so its
 * `skipped_undecryptable` replaces that page's rather than adding to it. That
 * page's *items* are still needed: rows the head has pushed out sit behind the
 * cursor page 2 was fetched with, so nothing else names them.
 */
function mergeHead(head: ItemPage, cached: { pages: ItemPage[] }): History {
  const merged = flatten([head, ...cached.pages]);
  return {
    items: merged.items,
    skipped: merged.skipped - (cached.pages[0]?.skipped_undecryptable ?? 0),
  };
}

/** Paged by cursor, never by offset (B-1, `CopyPaste-8ebg.57`). Search is not
 *  paged at all. */
export function useHistory(query: string, pushLive = false) {
  const searching = query.length > 0;

  const pages = useInfiniteQuery({
    queryKey: historyKey(query),
    queryFn: ({ pageParam }) =>
      searching
        ? searchItems(query, HISTORY_SEARCH_LIMIT)
        : listItems(HISTORY_PAGE_SIZE, pageParam),
    initialPageParam: null as string | null,
    // `next_cursor`, and nothing derived from the page's length: rows counted
    // in `skipped_undecryptable` were read and dropped, so stopping on a short
    // page hides the rest of the history behind a handful of corrupt rows.
    getNextPageParam: (lastPage) => (searching ? undefined : lastPage.next_cursor ?? undefined),
    // A periodic refetch on an infinite query re-walks `oldPages.length` pages
    // sequentially. The freshness the poll exists for only concerns the head,
    // which the query below polls on its own.
    refetchInterval: false,
    // Typing must not blank the list between keystrokes.
    placeholderData: keepPreviousData,
  });

  // INV-27: `refetchIntervalInBackground` stays unset, so a hidden window polls
  // zero times.
  const head = useQuery({
    queryKey: HISTORY_HEAD_KEY,
    queryFn: () => listItems(HISTORY_PAGE_SIZE, null),
    enabled: !searching,
    refetchInterval: (q) => pollInterval(pushLive, q.state.status === "error"),
  });

  const headPage = searching ? undefined : head.data;
  const cached = pages.data;

  // Both inputs are reference-stable across an idle poll, so this returns the
  // same list — INV-2, and the anchor INV-1/INV-6 reads is undisturbed.
  const data = useMemo(() => {
    if (cached === undefined) return headPage && flatten([headPage]);
    if (headPage === undefined) return flattenPages(cached);
    return mergeHead(headPage, cached);
  }, [cached, headPage]);

  return {
    data,
    // The head is the query on a timer, so it is the only one that sees the
    // service go away — and come back. Reporting the paged query's error here
    // left a stopped daemon rendering "Nothing copied yet" (bdac.2): nothing
    // refetches that query after the first load, so it never fails.
    error: searching ? pages.error : head.error,
    isError: searching ? pages.isError : head.isError,
    isPending: pages.isPending,
    hasNextPage: pages.hasNextPage,
    isFetchingNextPage: pages.isFetchingNextPage,
    fetchNextPage: pages.fetchNextPage,
    /** Both, because "try again" has to retry whichever one is failing. */
    async refetch() {
      const [paged, fresh] = await Promise.all([pages.refetch(), head.refetch()]);
      return { error: searching ? paged.error : fresh.error };
    },
  };
}

export function useHistorySearch(query: string) {
  return useQuery({
    queryKey: [...HISTORY_SEARCH_KEY, query],
    queryFn: () => searchItems(query, HISTORY_SEARCH_LIMIT),
    enabled: query.length > 0,
  });
}

export function historyOf(data: History | undefined): History {
  return data ?? EMPTY;
}
