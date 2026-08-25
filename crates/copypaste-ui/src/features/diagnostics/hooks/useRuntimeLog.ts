import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";

import {
  FOLLOW_PAGE_SIZE,
  type FollowResult,
  type KeyedEvent,
  followHead,
  mergeFollowed,
  unionEvents,
} from "@/features/diagnostics/model/runtimeLogRows";
import {
  type RuntimeLogEvent,
  type RuntimeLogLevel,
  type RuntimeLogProcess,
  getRuntimeLogEvents,
} from "@/service/runtimeLogs";

export const FOLLOW_POLL_MS = 3_000;

export interface RuntimeLogFilters {
  readonly query: string;
  readonly level: RuntimeLogLevel | "all";
  readonly process: RuntimeLogProcess | "all";
}

export interface RuntimeLogFeed {
  readonly rows: readonly KeyedEvent[];
  readonly events: readonly RuntimeLogEvent[];
  readonly isPending: boolean;
  readonly isError: boolean;
  /** The follow poll failed while the loaded rows are still on screen. */
  readonly followFailed: boolean;
  /** The live tail hit its page ceiling without reaching the held cache. Events
   *  in the gap are missing and a manual refetch is needed to recover. */
  readonly overrun: boolean;
  readonly hasNextPage: boolean;
  readonly isFetchingNextPage: boolean;
  readonly loadOlder: () => void;
  readonly refetch: () => void;
}

const NO_EVENTS: readonly RuntimeLogEvent[] = [];

export function useRuntimeLog(
  filters: RuntimeLogFilters,
  follow: boolean,
): RuntimeLogFeed {
  const queryKey = [
    "runtime-log-events",
    filters.query,
    filters.level,
    filters.process,
  ] as const;
  const request = {
    limit: FOLLOW_PAGE_SIZE,
    level: filters.level === "all" ? null : filters.level,
    process: filters.process === "all" ? null : filters.process,
    query: filters.query || null,
  };
  const loadingOlder = useRef(false);
  const [followed, setFollowed] = useState<readonly RuntimeLogEvent[]>(NO_EVENTS);
  const [overrun, setOverrun] = useState(false);
  const newestHeld = useRef<number | undefined>(undefined);

  const logs = useInfiniteQuery({
    queryKey,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) => getRuntimeLogEvents({ ...request, cursor: pageParam }),
    getNextPageParam: (page) => page.next_cursor,
    refetchInterval: false,
    retry: false,
  });

  const head = useQuery<FollowResult>({
    queryKey: [...queryKey, "head"],
    queryFn: () =>
      followHead(
        (cursor) => getRuntimeLogEvents({ ...request, cursor }),
        newestHeld.current,
      ),
    enabled: follow,
    refetchInterval: follow ? FOLLOW_POLL_MS : false,
    refetchIntervalInBackground: false,
    retry: false,
  });

  const signature = queryKey.join("\0");
  useEffect(() => {
    setFollowed(NO_EVENTS);
    setOverrun(false);
    newestHeld.current = undefined;
  }, [signature]);

  useEffect(() => {
    if (head.data === undefined) return;
    setFollowed((prev) => mergeFollowed(prev, head.data.events));
    if (head.data.overrun) setOverrun(true);
  }, [head.data]);

  const paged = useMemo(
    () => logs.data?.pages.flatMap((page) => page.events) ?? NO_EVENTS,
    [logs.data],
  );

  const rows = useMemo(() => unionEvents(followed, paged), [followed, paged]);
  const events = useMemo(() => rows.map((row) => row.event), [rows]);

  useEffect(() => {
    newestHeld.current = rows[0]?.event.timestamp_ms;
  }, [rows]);

  const loadOlder = useCallback(() => {
    if (!logs.hasNextPage || logs.isFetchingNextPage || loadingOlder.current) return;
    loadingOlder.current = true;
    void logs.fetchNextPage().finally(() => {
      loadingOlder.current = false;
    });
  }, [logs.fetchNextPage, logs.hasNextPage, logs.isFetchingNextPage]);

  const refetch = useCallback(() => {
    setOverrun(false);
    void logs.refetch();
    void head.refetch();
  }, [head.refetch, logs.refetch]);

  return {
    rows,
    events,
    isPending: logs.isPending,
    isError: logs.isError,
    followFailed: follow && head.isError && !logs.isError,
    overrun: follow && overrun && !logs.isError,
    hasNextPage: logs.hasNextPage,
    isFetchingNextPage: logs.isFetchingNextPage,
    loadOlder,
    refetch,
  };
}
