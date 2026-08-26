import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { toast } from "sonner";

import {
    ActionButton,
    IllustratedErrorState,
    InlineNotice,
    SearchField,
    SkeletonText,
} from "@/components/shared";
import { Button, Icon, Select, VisuallyHidden, type SelectItem } from "@/components/ui";
import { useRuntimeLog } from "@/features/diagnostics/hooks/useRuntimeLog";
import { t } from "@/i18n";
import { copyText } from "@/lib/ipc";
import { isAndroid } from "@/lib/platform";
import type {
    RuntimeLogEvent,
    RuntimeLogLevel,
    RuntimeLogProcess,
} from "@/service/runtimeLogs";
import styles from "./RuntimeLogViewer.module.css";

const LEVELS: readonly (RuntimeLogLevel | "all")[] = [
    "all",
    "error",
    "warn",
    "info",
    "debug",
    "trace",
];
const LEVEL_ICON = {
    all: "list",
    error: "alert",
    warn: "alert",
    info: "info",
    debug: "search",
    trace: "search",
} as const;
const PROCESS_ICON = {
    all: "devices",
    app: "app",
    daemon: "terminal",
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
    if (level === "error") return styles.levelError;
    if (level === "warn") return styles.levelWarning;
    if (level === "debug" || level === "trace") return styles.levelMuted;
    return styles.levelInfo;
}

export function RuntimeLogViewer() {
    const android = isAndroid();
    const [query, setQuery] = useState("");
    const [level, setLevel] = useState<RuntimeLogLevel | "all">("all");
    const [process, setProcess] = useState<RuntimeLogProcess | "all">("all");
    const [follow, setFollow] = useState(true);
    const scrollRef = useRef<HTMLDivElement>(null);
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
        {
            value: "all",
            label: t("runtimeLog.process.all"),
            icon: PROCESS_ICON.all,
        },
        {
            value: "app",
            label: t("runtimeLog.process.app"),
            icon: PROCESS_ICON.app,
        },
        {
            value: "daemon",
            label: t("runtimeLog.process.daemon"),
            icon: PROCESS_ICON.daemon,
        },
    ];

    const virtualizer = useVirtualizer({
        count: rows.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => 48,
        getItemKey: (index) => rows[index]?.key ?? index,
        overscan: 8,
        useFlushSync: false,
    });

    useEffect(() => {
        if (follow && events.length > 0)
            virtualizer.scrollToIndex(0, { align: "start" });
    }, [events.length, follow, virtualizer]);

    const copyLoaded = () => {
        void copyText(events.map(rowText).join("\n"))
            .then(() => toast.success(t("runtimeLog.toast.loadedCopied")))
            .catch(() => toast.error(t("runtimeLog.toast.loadedCopyFailed")));
    };

    const handleScroll = () => {
        const element = scrollRef.current;
        if (!element) return;
        if (element.scrollTop > 24 && follow) setFollow(false);
        if (
            element.scrollHeight - element.scrollTop - element.clientHeight <
            240
        ) {
            loadOlder();
        }
    };

    return (
        <section aria-label={t("runtimeLog.title")} className={styles.root}>
            <div className={styles.toolbar}>
                <div className={styles.search}>
                    <SearchField
                        size="sm"
                        shortcut=""
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        onClear={() => setQuery("")}
                        placeholder={t("runtimeLog.search.label")}
                        aria-label={t("runtimeLog.search.label")}
                    />
                </div>
                <VisuallyHidden asChild>
                    <label htmlFor="runtime-log-level">
                        {t("runtimeLog.level.label")}
                    </label>
                </VisuallyHidden>
                <Select
                    id="runtime-log-level"
                    value={level}
                    items={levelItems}
                    onValueChange={(next) =>
                        setLevel(next as RuntimeLogLevel | "all")
                    }
                    size="sm"
                    measure="auto"
                    presentation="auto"
                    aria-label={t("runtimeLog.level.label")}
                />
                {!android && (
                    <>
                        <VisuallyHidden asChild>
                            <label htmlFor="runtime-log-process">
                                {t("runtimeLog.process.label")}
                            </label>
                        </VisuallyHidden>
                        <Select
                            id="runtime-log-process"
                            value={process}
                            items={processItems}
                            onValueChange={(next) =>
                                setProcess(next as RuntimeLogProcess | "all")
                            }
                            size="sm"
                            measure="auto"
                            presentation="auto"
                            aria-label={t("runtimeLog.process.label")}
                        />
                    </>
                )}
                <ActionButton
                    size="compactIcon"
                    icon="refresh"
                    onClick={logs.refetch}
                    aria-label={t("runtimeLog.refresh")}
                    title={t("runtimeLog.refresh")}
                />
                <ActionButton
                    size="compactIcon"
                    icon={follow ? "pause" : "play"}
                    onClick={() => setFollow((value) => !value)}
                    aria-pressed={follow}
                    aria-label={t(
                        follow ? "runtimeLog.pause" : "runtimeLog.resume",
                    )}
                    title={t(follow ? "runtimeLog.pause" : "runtimeLog.resume")}
                />
                <ActionButton
                    size="compactIcon"
                    icon="copy"
                    onClick={copyLoaded}
                    disabled={events.length === 0}
                    aria-label={t("runtimeLog.copyLoaded")}
                    title={t("runtimeLog.copyLoaded")}
                />
            </div>

            {logs.overrun && (
                <InlineNotice role="alert" tone="warning" icon="alert">
                    {t("runtimeLog.overrun")}
                </InlineNotice>
            )}

            {logs.followFailed && (
                <InlineNotice role="alert" tone="danger" icon="alert">
                    {t("runtimeLog.followFailed")}
                </InlineNotice>
            )}

            {logs.isPending ? (
                <div
                    className={styles.logSkeleton}
                    role="status"
                    aria-label={`${t("runtimeLog.title")}…`}
                    aria-busy="true"
                >
                    {Array.from({ length: 24 }, (_, index) => (
                        <article
                            className={`${styles.row} ${styles.skeletonRow}`}
                            key={index}
                            aria-hidden="true"
                        >
                            <time className={styles.time}>
                                <SkeletonText width="fill" />
                            </time>
                            <div className={styles.rowContent}>
                                <div className={styles.metadata}>
                                    <SkeletonText width="xs" />
                                    <SkeletonText width="xs" />
                                    <SkeletonText width="sm" />
                                </div>
                                <p className={styles.message}>
                                    <SkeletonText
                                        width={index % 3 === 0 ? "fill" : "md"}
                                    />
                                </p>
                            </div>
                        </article>
                    ))}
                </div>
            ) : logs.isError ? (
                <IllustratedErrorState
                    compact
                    title={t("runtimeLog.loadFailed.title")}
                    body={t("runtimeLog.loadFailed.body")}
                    actions={
                        <Button size="sm" onClick={logs.refetch}>
                            <Icon name="refresh" size="sm" />
                            {t("common.tryAgain")}
                        </Button>
                    }
                />
            ) : events.length === 0 ? (
                <div
                    className={styles.emptyState}
                    data-runtime-log-empty
                    role="status"
                >
                    <Icon name="searchX" size="lg" className={styles.emptyIcon} />
                    <p className={styles.emptyCopy}>{t("runtimeLog.noMatch")}</p>
                </div>
            ) : (
                <>
                    <div
                        ref={scrollRef}
                        role="log"
                        aria-live="off"
                        aria-label={t("runtimeLog.list")}
                        onScroll={handleScroll}
                        className={styles.log}
                    >
                        <div
                            className={styles.virtualCanvas}
                            style={{
                                height: logs.hasNextPage
                                    ? `calc(${virtualizer.getTotalSize()}px + var(--ctl-h-sm))`
                                    : virtualizer.getTotalSize(),
                            }}
                        >
                            {virtualizer.getVirtualItems().map((virtualRow) => {
                                const event = events[virtualRow.index];
                                if (!event) return null;
                                return (
                                    <article
                                        key={virtualRow.key}
                                        ref={virtualizer.measureElement}
                                        data-index={virtualRow.index}
                                        className={styles.row}
                                        style={{
                                            transform: `translateY(${virtualRow.start}px)`,
                                        }}
                                    >
                                        <time
                                            className={styles.time}
                                            dateTime={new Date(
                                                event.timestamp_ms,
                                            ).toISOString()}
                                        >
                                            {timeLabel(event.timestamp_ms)}
                                        </time>
                                        <div className={styles.rowContent}>
                                            <div className={styles.metadata}>
                                                <span
                                                    className={levelClass(
                                                        event.level,
                                                    )}
                                                >
                                                    {event.level.toUpperCase()}
                                                </span>
                                                <span
                                                    className={styles.process}
                                                >
                                                    {t(
                                                        event.process ===
                                                            "daemon"
                                                            ? "runtimeLog.process.daemon"
                                                            : "runtimeLog.process.app",
                                                    )}
                                                </span>
                                                <span
                                                    className={styles.target}
                                                    title={event.target}
                                                >
                                                    {event.target}
                                                </span>
                                            </div>
                                            <p className={styles.message}>
                                                {event.message}
                                            </p>
                                        </div>
                                    </article>
                                );
                            })}
                            {logs.hasNextPage && (
                                <div
                                    className={styles.olderEvents}
                                    style={{
                                        transform: `translateY(${virtualizer.getTotalSize()}px)`,
                                    }}
                                    role={logs.isFetchingNextPage ? "status" : undefined}
                                    aria-label={
                                        logs.isFetchingNextPage
                                            ? t("runtimeLog.loadingOlder")
                                            : undefined
                                    }
                                >
                                    {logs.isFetchingNextPage && (
                                        <SkeletonText width="sm" />
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
