import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  AppWindow,
  ClipboardCopy,
  ListChecks,
  LoaderCircle,
  Pause,
  Radio,
  RefreshCw,
  Search,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, type SelectItem } from "@/components/ui/select";
import { useRuntimeLog } from "@/components/diagnostics/useRuntimeLog";
import { copyText } from "@/lib/ipc";
import { t } from "@/i18n";
import { isAndroid } from "@/lib/platform";
import type { RuntimeLogEvent, RuntimeLogLevel, RuntimeLogProcess } from "@/service/runtimeLogs";

const LEVELS: readonly (RuntimeLogLevel | "all")[] = [
  "all",
  "error",
  "warn",
  "info",
  "debug",
  "trace",
];
const LEVEL_ICON = {
  all: ListChecks,
  error: TriangleAlert,
  warn: TriangleAlert,
  info: Radio,
  debug: Search,
  trace: Search,
} as const;
const PROCESS_ICON = {
  all: ListChecks,
  app: AppWindow,
  daemon: Radio,
} as const;

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
  const deferredQuery = useDeferredValue(query);

  const filters = useMemo(
    () => ({ query: deferredQuery, level, process }),
    [deferredQuery, level, process],
  );
  const logs = useRuntimeLog(filters, follow);
  const { events, loadOlder, rows } = logs;
  const levelItems: readonly SelectItem[] = LEVELS.map((option) => ({
    value: option,
    label: t(`runtimeLog.level.${option}`),
    icon: LEVEL_ICON[option],
  }));
  const processItems: readonly SelectItem[] = [
    { value: "all", label: t("runtimeLog.process.all"), icon: PROCESS_ICON.all },
    { value: "app", label: t("runtimeLog.process.app"), icon: PROCESS_ICON.app },
    { value: "daemon", label: t("runtimeLog.process.daemon"), icon: PROCESS_ICON.daemon },
  ];

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 68,
    getItemKey: (index) => rows[index]?.key ?? index,
    overscan: 8,
    useFlushSync: false,
  });

  useEffect(() => {
    if (follow && events.length > 0) virtualizer.scrollToIndex(0, { align: "start" });
  }, [events.length, follow, virtualizer]);

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
      .then(() => toast.success(t("runtimeLog.toast.entryCopied")))
      .catch(() => toast.error(t("runtimeLog.toast.entryCopyFailed")));
  };

  const copyLoaded = () => {
    void copyText(events.map(rowText).join("\n"))
      .then(() => toast.success(t("runtimeLog.toast.loadedCopied")))
      .catch(() => toast.error(t("runtimeLog.toast.loadedCopyFailed")));
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
    <section
      aria-label={t("runtimeLog.title")}
      className="flex h-full min-h-0 flex-1 flex-col gap-s-3"
    >
      <div className={android ? "flex flex-wrap items-center gap-s-2" : "flex flex-nowrap items-center gap-s-2"}>
        <label className="sr-only" htmlFor="runtime-log-search">
          {t("runtimeLog.search.label")}
        </label>
        <Input
          id="runtime-log-search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("runtimeLog.search.label")}
          className={android ? "min-w-48 flex-1 bg-panel" : "min-w-0 flex-1 bg-panel"}
        />
        <label className="sr-only" htmlFor="runtime-log-level">
          {t("runtimeLog.level.label")}
        </label>
        <Select
          id="runtime-log-level"
          value={level}
          items={levelItems}
          onValueChange={(next) => setLevel(next as RuntimeLogLevel | "all")}
          className="w-36"
        />
        {!android && (
          <>
            <label className="sr-only" htmlFor="runtime-log-process">
              {t("runtimeLog.process.label")}
            </label>
            <Select
              id="runtime-log-process"
              value={process}
              items={processItems}
              onValueChange={(next) => setProcess(next as RuntimeLogProcess | "all")}
              className="w-42"
            />
          </>
        )}
        <Button
          size="icon-sm"
          variant="ghost"
          onClick={logs.refetch}
          aria-label={t("runtimeLog.refresh")}
          title={t("runtimeLog.refresh")}
        >
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
          aria-label={t(follow ? "runtimeLog.pause" : "runtimeLog.resume")}
          title={t(follow ? "runtimeLog.pause" : "runtimeLog.resume")}
        >
          {follow ? <Radio aria-hidden="true" /> : <Pause aria-hidden="true" />}
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          onClick={copyLoaded}
          disabled={events.length === 0}
          aria-label={t("runtimeLog.copyLoaded")}
          title={t("runtimeLog.copyLoaded")}
        >
          <ClipboardCopy aria-hidden="true" />
        </Button>
      </div>

      {logs.overrun && (
        <div
          role="alert"
          className="rounded-lg border border-divider bg-panel px-s-3 py-s-2 text-sm text-warn-strong"
        >
          {t("runtimeLog.overrun")}
        </div>
      )}

      {logs.followFailed && (
        <div
          role="alert"
          className="rounded-lg border border-divider bg-panel px-s-3 py-s-2 text-sm text-err-strong"
        >
          {t("runtimeLog.followFailed")}
        </div>
      )}

      {logs.isPending ? (
        <div className="flex min-h-40 items-center justify-center gap-s-2 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />{" "}
          {t("runtimeLog.loading")}
        </div>
      ) : logs.isError ? (
        <div role="alert" className="flex min-h-40 items-center justify-center text-sm text-err-strong">
          {t("runtimeLog.loadFailed")}
        </div>
      ) : events.length === 0 ? (
        <div className="flex min-h-40 items-center justify-center text-sm text-muted-foreground">
          {t("runtimeLog.noMatch")}
        </div>
      ) : (
        <>
          <div
            ref={scrollRef}
            role="log"
            aria-live="off"
            aria-label={t("runtimeLog.list")}
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
                        <span className="text-muted-foreground">
                        {t(event.process === "daemon" ? "runtimeLog.process.daemon" : "runtimeLog.process.app")}
                      </span>
                        <span className="truncate font-mono text-muted-foreground">{event.target}</span>
                      </div>
                      <p className="mt-0.5 break-words text-sm text-foreground">{event.message}</p>
                    </div>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      className="transition-opacity [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 [@media(hover:hover)]:group-focus-within:opacity-100"
                      onClick={() => copy(event)}
                      aria-label={t("runtimeLog.copyEntry")}
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
                    <>
                      <LoaderCircle className="mr-1.5 size-3 animate-spin" aria-hidden="true" />{" "}
                      {t("runtimeLog.loadingOlder")}
                    </>
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
