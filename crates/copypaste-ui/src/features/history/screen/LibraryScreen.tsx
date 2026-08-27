import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { CaptureStatus } from "@/features/capture";
import { Container, Screen, SplitPane } from "@/components/layout";
import { HistoryDialogs } from "@/features/history/patterns/HistoryDialogs";
import { RevealNotice } from "@/features/history/patterns/RevealNotice";
import { SkippedNotice } from "@/features/history/patterns/SkippedNotice";
import { ClipDetailDialog } from "@/features/history/patterns/ClipDetailDialog";
import { HistoryContentState } from "@/features/history/patterns/HistoryContentState";
import { LibraryInspectorPanel } from "@/features/history/patterns/LibraryInspectorPanel";
import { LibraryToolbar } from "@/features/history/patterns/LibraryToolbar";
import { markedOrigin, markedOrigins } from "@/features/history/model/origin";
import { HISTORY_LAYOUT_METRICS } from "@/features/history/model/virtualizationMetrics";
import {
    useCopy,
    usePin,
    useReorderPinned,
} from "@/features/history/hooks/useHistoryMutations";
import { useStatus } from "@/hooks/useStatus";
import type { StatusData } from "@/lib/ipc";
import { useHistoryController } from "@/features/history/hooks/useHistoryController";
import { useHistorySelection } from "@/features/history/hooks/useHistorySelection";
import { useReveal } from "@/features/history/hooks/useReveal";
import { t } from "@/i18n";
import type { Item } from "@/lib/ipc";
import { useItemBody } from "@/hooks/useItemBody";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { useViewportMetrics } from "@/hooks/useViewportMetrics";
import styles from "./LibraryScreen.module.css";

const captureModes = (data: StatusData) => ({
    privateMode: data.private_mode === true,
    capturePaused: data.capture_running === false,
});

const INSPECTOR_SIZE_KEY = "copypaste.library.inspector-width";
const INSPECTOR_OPEN_KEY = "copypaste.library.inspector-open";

function initialInspectorSize(): number {
    const { defaultPx, minPx } = HISTORY_LAYOUT_METRICS.inspector;
    if (typeof window === "undefined") return defaultPx;
    try {
        const stored = Number(
            window.sessionStorage.getItem(INSPECTOR_SIZE_KEY),
        );
        if (Number.isFinite(stored) && stored >= minPx) return stored;
    } catch {
        return defaultPx;
    }
    return defaultPx;
}

function initialInspectorOpen(): boolean {
    if (typeof window === "undefined") return true;
    try {
        return window.sessionStorage.getItem(INSPECTOR_OPEN_KEY) !== "false";
    } catch {
        return true;
    }
}

const pixels = (value: number): `${number}px` => `${value}px`;

function persistInspectorSize(pixels: number): void {
    try {
        window.sessionStorage.setItem(
            INSPECTOR_SIZE_KEY,
            String(Math.round(pixels)),
        );
    } catch {
        return;
    }
}

function persistInspectorOpen(open: boolean): void {
    try {
        window.sessionStorage.setItem(INSPECTOR_OPEN_KEY, String(open));
    } catch {
        return;
    }
}

interface LibraryScreenProps {
    /** From `usePush` at the app root: slows the poll without stopping it. */
    pushLive?: boolean;
}

export function LibraryScreen({ pushLive = false }: LibraryScreenProps) {
    const activeId = useUi((s) => s.activeId);
    const setActiveId = useUi((s) => s.setActiveId);
    const setView = useUi((s) => s.setView);
    const setSettingsTab = useUi((s) => s.setSettingsTab);
    const previewLines = usePrefs((s) => s.previewLines);

    const searchRef = useRef<HTMLInputElement>(null);
    const listRef = useRef<HTMLDivElement>(null);
    const desktopInspector =
        useViewportMetrics().width >=
        HISTORY_LAYOUT_METRICS.inspector.visibleAtPx;

    const history = useHistoryController(pushLive);
    const bulk = useHistorySelection(history.items);
    const reveal = useReveal();
    const copy = useCopy();
    const pin = usePin();
    const reorder = useReorderPinned();
    const status = useStatus(captureModes);

    const [detailId, setDetailId] = useState<string | null>(null);
    const [inspectorOpen, setInspectorOpen] = useState(initialInspectorOpen);
    const [inspectorSize] = useState(initialInspectorSize);
    const [optimisticPinned, setOptimisticPinned] = useState<
        readonly string[] | null
    >(null);

    const selection = bulk.selection;
    const items = useMemo(() => {
        if (optimisticPinned === null) return history.items;
        const positions = new Map(
            optimisticPinned.map((id, index) => [id, index]),
        );
        const pinned = history.items.filter((item) => item.pinned);
        pinned.sort(
            (a, b) =>
                (positions.get(a.id) ?? Number.MAX_SAFE_INTEGER) -
                (positions.get(b.id) ?? Number.MAX_SAFE_INTEGER),
        );
        let nextPinned = 0;
        return history.items.map((item) =>
            item.pinned ? pinned[nextPinned++]! : item,
        );
    }, [history.items, optimisticPinned]);

    const reorderPinned = useCallback(
        (ids: readonly string[]) => {
            setOptimisticPinned(ids);
            reorder.mutate(ids, {
                onError: () => setOptimisticPinned(null),
                onSuccess: () => setOptimisticPinned(null),
            });
        },
        [reorder],
    );

    useEffect(() => {
        if (activeId === null) return;
        if (!items.some((item) => item.id === activeId))
            setActiveId(items[0]?.id ?? null);
    }, [activeId, items, setActiveId]);

    const originsMarked = useMemo(() => markedOrigins(items), [items]);
    const inspected = useMemo(
        () => items.find((item) => item.id === activeId) ?? null,
        [activeId, items],
    );

    // Resolved from the id on every render, never held: a poll can replace the
    // array while the view is open, and an item deleted underneath the reader
    // closes it rather than leaving a copy of a row that no longer exists.
    const detail = useMemo(() => {
        if (detailId === null) return null;
        const item = items.find((candidate) => candidate.id === detailId);
        return item
            ? { item, origin: markedOrigin(item, originsMarked) }
            : null;
    }, [detailId, items, originsMarked]);

    const detailBody = useItemBody(
        detail?.item ?? (desktopInspector && inspectorOpen ? inspected : null),
    );

    const openDetail = useCallback(
        (item: Item) => {
            setActiveId(item.id);
            setDetailId(item.id);
        },
        [setActiveId],
    );
    const changeActiveId = useCallback(
        (id: string | null) => {
            setActiveId(id);
            if (id !== null && desktopInspector) {
                setInspectorOpen(true);
                persistInspectorOpen(true);
            }
        },
        [desktopInspector, setActiveId],
    );
    const closeInspector = useCallback(() => {
        setInspectorOpen(false);
        persistInspectorOpen(false);
        requestAnimationFrame(() => {
            const row = activeId
                ? document.getElementById(`history-row-${activeId}`)
                : null;
            row?.querySelector<HTMLButtonElement>("button")?.focus();
            if (!row) listRef.current?.focus();
        });
    }, [activeId]);

    const primary = (
        <Screen className={styles.main}>
            <LibraryToolbar
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
                selection={
                    selection.active
                        ? {
                              count: selection.items.length,
                              total: items.length,
                              allSelected:
                                  items.length > 0 &&
                                  selection.selected.size === items.length,
                              allPinned: selection.allPinned,
                              busy: bulk.busy,
                              onToggleAll: () => {
                                  if (
                                      selection.selected.size === items.length
                                  ) {
                                      selection.clear();
                                  } else {
                                      selection.selectAll();
                                  }
                              },
                              onSelectAll: selection.selectAll,
                              onTogglePin: bulk.togglePin,
                              onDelete: bulk.requestDelete,
                              onClose: bulk.end,
                          }
                        : undefined
                }
            />

            <CaptureStatus />
            <SkippedNotice count={history.skipped} />

            <Container width="library" gutter="screen" asChild>
                <section
                    className={styles.stream}
                    aria-label={t("history.stream.label")}
                >
                    <HistoryContentState
                        loading={history.loading}
                        errorKind={history.errorKind}
                        searching={history.searching}
                        filtered={history.filtered}
                        privateMode={status.data?.privateMode === true}
                        capturePaused={status.data?.capturePaused === true}
                        query={history.query}
                        hasMore={history.hasMore}
                        onLoadMore={history.loadMore}
                        onRetry={history.retry}
                        onOpenCapture={() => setView("capture")}
                        onOpenDiagnostics={() => {
                            setSettingsTab("diagnostics");
                            setView("settings");
                        }}
                        list={{
                            items,
                            activeId,
                            onActiveIdChange: changeActiveId,
                            revealedId: reveal.revealedId,
                            revealedContent: reveal.revealedContent,
                            revealPendingId: reveal.pendingId,
                            previewLines,
                            groupedByDevice: history.groupedByDevice,
                            selection,
                            hasMore: history.hasMore,
                            loadingMore: history.loadingMore,
                            onReorderPinned: reorderPinned,
                            onDelete: history.remove,
                            onOpen: openDetail,
                            onLoadMore: history.loadMore,
                            listRef,
                        }}
                    />
                </section>
            </Container>

            <RevealNotice message={reveal.error} />

            <ClipDetailDialog
                item={detail?.item ?? null}
                origin={detail?.origin ?? null}
                initialExpanded={false}
                fullContent={detailBody.text}
                fullContentFailed={detailBody.failed}
                revealedContent={
                    detail && reveal.revealedId === detail.item.id
                        ? reveal.revealedContent
                        : null
                }
                revealPending={reveal.pendingId === detailId}
                onReveal={reveal.request}
                onHide={reveal.hide}
                onCopy={copy.mutate}
                onTogglePin={pin.mutate}
                onDelete={history.remove}
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
            />
        </Screen>
    );

    const inspector =
        desktopInspector && inspectorOpen ? (
            <LibraryInspectorPanel
                item={inspected}
                origin={
                    inspected ? markedOrigin(inspected, originsMarked) : null
                }
                revealedContent={
                    inspected?.id === reveal.revealedId
                        ? reveal.revealedContent
                        : null
                }
                fullContent={detailBody.text}
                fullContentFailed={detailBody.failed}
                revealPending={inspected?.id === reveal.pendingId}
                onReveal={reveal.request}
                onHide={reveal.hide}
                onCopy={copy.mutate}
                onTogglePin={pin.mutate}
                onDelete={history.remove}
                onClose={closeInspector}
            />
        ) : undefined;

    return (
        <Screen className={styles.screen}>
            <SplitPane
                primary={primary}
                secondary={inspector}
                primaryId="library-stream"
                secondaryId="library-inspector"
                primaryMinSize={
                    desktopInspector
                        ? pixels(HISTORY_LAYOUT_METRICS.inspector.primaryMinPx)
                        : 0
                }
                secondaryDefaultSize={pixels(inspectorSize)}
                secondarySize={pixels(inspectorSize)}
                secondaryMinSize={pixels(
                    HISTORY_LAYOUT_METRICS.inspector.minPx,
                )}
                secondaryMaxSize={HISTORY_LAYOUT_METRICS.inspector.maxSize}
                separatorLabel={t("history.inspector.resize")}
                onSecondarySizeChange={persistInspectorSize}
            />
        </Screen>
    );
}
