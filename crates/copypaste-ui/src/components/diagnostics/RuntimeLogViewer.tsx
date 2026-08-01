import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ClipboardCopy, LoaderCircle, Pause, Radio, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NativeSelect } from "@/components/ui/native-select";
import { copyText } from "@/lib/ipc";
import { isAndroid } from "@/lib/platform";
import {
  getRuntimeLogEvents,
  type RuntimeLogEvent,
  type RuntimeLogLevel,
  type RuntimeLogProcess,
} from "@/service/runtimeLogs";

const PAGE_SIZE = 50;

const LEVELS: readonly { value: RuntimeLogLevel | "all"; label: string }[] = [
  { value: "all", label: "All levels" },
  { value: "error", label: "Errors" },
  { value: "warn", label: "Warnings" },
  { value: "info", label: "Info" },
  { value: "debug", label: "Debug" },
  { value: "trace", label: "Trace" },
];

function timeLabel(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(timestamp);
}

function rowText(event: RuntimeLogEvent): string {
  return `${new Date(event.timestamp_ms).toISOString()} ${event.level.toUpperCase()} ${event.process} ${event.target}: ${event.message}`;
}

function levelClass(level: RuntimeLogLevel): string {
  if (level === "error") return "text-err-strong";
  if (level === "warn") return "text-warn-strong";
  if (level === "debug" || level === "trace") return "text-muted-foreground";
  return "text-brand-2";
}

export function RuntimeLogViewer() {
  const android = isAndroid();
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState<RuntimeLogLevel | "all">("all");
  const [process, setProcess] = useState<RuntimeLogProcess | "all">("all");
  const [follow, setFollow] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const olderEventsSentinelRef = useRef<HTMLDivElement>(null);
  const loadingOlderRef = useRef(false);
  const deferredQuery = useDeferredValue(query);
  const queryKey = ["runtime-log-events", deferredQuery, level, process] as const;
  const logs = useInfiniteQuery({
    queryKey,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      getRuntimeLogEvents({
        cursor: pageParam,
        limit: PAGE_SIZE,
        level: level === "all" ? null : level,
        process: process === "all" ? null : process,
        query: deferredQuery || null,
      }),
    getNextPageParam: (page) => page.next_cursor,
    refetchInterval: follow ? 3_000 : false,
    refetchIntervalInBackground: false,
    retry: false,
  });
  const events = useMemo(
    () => logs.data?.pages.flatMap((page) => page.events) ?? [],
    [logs.data],
  );
  const virtualizer = useVirtualizer({
    count: events.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 68,
    getItemKey: (index) => `${events[index]?.process}:${events[index]?.timestamp_ms}:${index}`,
    overscan: 8,
    useFlushSync: false,
  });

  useEffect(() => {
    if (follow && events.length > 0) virtualizer.scrollToIndex(0, { align: "start" });
  }, [events.length, follow, virtualizer]);

  const loadOlder = useCallback(() => {
    if (!logs.hasNextPage || logs.isFetchingNextPage || loadingOlderRef.current) return;
    loadingOlderRef.current = true;
    void logs.fetchNextPage().finally(() => {
      loadingOlderRef.current = false;
    });
  }, [logs.fetchNextPage, logs.hasNextPage, logs.isFetchingNextPage]);

  useEffect(() => {
    const root = scrollRef.current;
    const sentinel = olderEventsSentinelRef.current;
    if (!root || !sentinel || !logs.hasNextPage || typeof IntersectionObserver === "undefined") return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) loadOlder();
      },
      { root, rootMargin: "0px 0px 240px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [events.length, loadOlder, logs.hasNextPage]);

  const copy = (event: RuntimeLogEvent) => {
    void copyText(rowText(event))
      .then(() => toast.success("Log entry copied"))
      .catch(() => toast.error("Couldn’t copy this log entry."));
  };

  const copyLoaded = () => {
    void copyText(events.map(rowText).join("\n"))
      .then(() => toast.success("Loaded log events copied"))
      .catch(() => toast.error("Couldn’t copy loaded log events."));
  };

  const handleScroll = () => {
    const element = scrollRef.current;
    if (!element) return;
    if (element.scrollTop > 24 && follow) setFollow(false);
    if (element.scrollHeight - element.scrollTop - element.clientHeight < 240) {
      loadOlder();
    }
  };

  return (
    <section aria-label="Runtime events" className="flex h-full min-h-0 flex-1 flex-col gap-s-3">
      <div className={android ? "flex flex-wrap items-center gap-s-2" : "flex flex-nowrap items-center gap-s-2"}>
        <label className="sr-only" htmlFor="runtime-log-search">Search runtime events</label>
        <Input
          id="runtime-log-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search runtime events"
          className={android ? "min-w-48 flex-1 bg-panel" : "min-w-0 flex-1 bg-panel"}
        />
        <label className="sr-only" htmlFor="runtime-log-level">Level</label>
        <NativeSelect
          id="runtime-log-level"
          value={level}
          onChange={(event) => setLevel(event.target.value as RuntimeLogLevel | "all")}
          className="w-36"
        >
          {LEVELS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
        </NativeSelect>
        {!android && (
          <>
            <label className="sr-only" htmlFor="runtime-log-process">Process</label>
            <NativeSelect
              id="runtime-log-process"
              value={process}
              onChange={(event) => setProcess(event.target.value as RuntimeLogProcess | "all")}
              className="w-42"
            >
              <option value="all">All processes</option>
              <option value="app">App</option>
              <option value="daemon">Service</option>
            </NativeSelect>
          </>
        )}
        <Button size="icon-sm" variant="ghost" onClick={() => void logs.refetch()} aria-label="Refresh runtime events" title="Refresh runtime events">
          <RefreshCw aria-hidden="true" />
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          className={follow
            ? "bg-transparent text-brand-2 hover:bg-transparent hover:text-brand-2"
            : "bg-transparent text-muted-foreground hover:bg-transparent hover:text-foreground"}
          onClick={() => setFollow((value) => !value)}
          aria-pressed={follow}
          aria-label={follow ? "Pause live updates" : "Resume live updates"}
          title={follow ? "Pause live updates" : "Resume live updates"}
        >
          {follow ? <Radio aria-hidden="true" /> : <Pause aria-hidden="true" />}
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          onClick={copyLoaded}
          disabled={events.length === 0}
          aria-label="Copy loaded log events"
          title="Copy loaded log events"
        >
          <ClipboardCopy aria-hidden="true" />
        </Button>
      </div>

      {logs.isPending ? (
        <div className="flex min-h-40 items-center justify-center gap-s-2 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin" aria-hidden="true" /> Loading runtime events…
        </div>
      ) : logs.isError ? (
        <div role="alert" className="flex min-h-40 items-center justify-center text-sm text-err-strong">
          Runtime events couldn’t be loaded.
        </div>
      ) : events.length === 0 ? (
        <div className="flex min-h-40 items-center justify-center text-sm text-muted-foreground">
          No runtime events match these filters.
        </div>
      ) : (
        <>
          <div
            ref={scrollRef}
            role="log"
            aria-live="off"
            aria-label="Runtime event list"
            onScroll={handleScroll}
            className="min-h-0 flex-1 overflow-y-auto rounded-lg border border-divider bg-panel"
          >
            <div
              className="relative w-full"
              style={{ height: virtualizer.getTotalSize() + (logs.hasNextPage ? 32 : 0) }}
            >
              {virtualizer.getVirtualItems().map((virtualRow) => {
                const event = events[virtualRow.index];
                if (!event) return null;
                return (
                  <article
                    key={virtualRow.key}
                    ref={virtualizer.measureElement}
                    data-index={virtualRow.index}
                    className="group absolute left-0 top-0 flex w-full items-start gap-s-2 border-b border-divider px-s-3 py-s-2 last:border-b-0"
                    style={{ transform: `translateY(${virtualRow.start}px)` }}
                  >
                    <time className="w-16 shrink-0 pt-0.5 text-xs tabular-nums text-muted-foreground" dateTime={new Date(event.timestamp_ms).toISOString()}>
                      {timeLabel(event.timestamp_ms)}
                    </time>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs">
                        <span className={levelClass(event.level)}>{event.level.toUpperCase()}</span>
                        <span className="text-muted-foreground">{event.process === "daemon" ? "Service" : "App"}</span>
                        <span className="truncate font-mono text-muted-foreground">{event.target}</span>
                      </div>
                      <p className="mt-0.5 break-words text-sm text-foreground">{event.message}</p>
                    </div>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      className="transition-opacity [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 [@media(hover:hover)]:group-focus-within:opacity-100"
                      onClick={() => copy(event)}
                      aria-label="Copy log entry"
                    >
                      <ClipboardCopy aria-hidden="true" />
                    </Button>
                  </article>
                );
              })}
              {logs.hasNextPage && (
                <div
                  ref={olderEventsSentinelRef}
                  className="absolute inset-x-0 flex h-8 items-center justify-center text-xs text-muted-foreground"
                  style={{ transform: `translateY(${virtualizer.getTotalSize()}px)` }}
                  aria-live="polite"
                >
                  {logs.isFetchingNextPage && (
                    <><LoaderCircle className="mr-1.5 size-3 animate-spin" aria-hidden="true" /> Loading older events…</>
                  )}
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
